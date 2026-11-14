use crate::config::load_state;
use crate::docker::{connect, DockerLease};
use crate::error::{respond, ApiResponse, AppError};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use bollard::container::LogOutput;
use bollard::exec::{CreateExecOptions, ResizeExecOptions, StartExecOptions, StartExecResults};
use futures_util::StreamExt;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

#[derive(Default)]
pub struct TerminalManager {
    sessions: Mutex<HashMap<String, TerminalSession>>,
}

struct TerminalSession {
    run_id: String,
    node_id: String,
    container_id: String,
    lease: DockerLease,
    exec_id: String,
    input: mpsc::Sender<Vec<u8>>,
    cancel: Option<oneshot::Sender<()>>,
}

impl TerminalManager {
    fn lock(&self) -> MutexGuard<'_, HashMap<String, TerminalSession>> {
        self.sessions
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn remove_if(&self, session_id: &str, run_id: &str) -> bool {
        let mut sessions = self.lock();
        if sessions
            .get(session_id)
            .is_some_and(|session| session.run_id == run_id)
        {
            sessions.remove(session_id);
            true
        } else {
            false
        }
    }

    pub fn close_all(&self) {
        let sessions = {
            let mut current = self.lock();
            current
                .drain()
                .map(|(_, session)| session)
                .collect::<Vec<_>>()
        };
        for mut session in sessions {
            if let Some(cancel) = session.cancel.take() {
                let _ = cancel.send(());
            }
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOpenResult {
    pub session_id: String,
    pub reused: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalOutputEvent {
    session_id: String,
    seq: u64,
    data: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalStatusEvent {
    session_id: String,
    seq: u64,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

#[tauri::command]
pub async fn terminal_open(
    app: AppHandle,
    node_id: String,
    container_id: String,
    session_id: Option<String>,
    command: Option<Vec<String>>,
    cols: Option<u16>,
    rows: Option<u16>,
) -> ApiResponse<TerminalOpenResult> {
    respond(
        async {
            if node_id.trim().is_empty() || container_id.trim().is_empty() {
                return Err(AppError::input("舰船和集装箱标识不能为空。"));
            }
            let session_id = session_id
                .and_then(|value| {
                    let value = value.trim().to_string();
                    (!value.is_empty()).then_some(value)
                })
                .unwrap_or_else(|| Uuid::new_v4().to_string());
            if session_id.len() > 128 {
                return Err(AppError::input("终端会话编号过长。"));
            }
            let manager = app.state::<TerminalManager>();
            if let Some(result) = existing_session(&manager, &session_id, &node_id, &container_id)?
            {
                return Ok(result);
            }

            let command = command.unwrap_or_else(|| vec!["/bin/sh".to_string()]);
            if command.is_empty() || command[0].trim().is_empty() {
                return Err(AppError::input("终端命令不能为空。"));
            }
            let cols = valid_size(cols.unwrap_or(80), rows.unwrap_or(24))?;
            let node = load_state()?
                .nodes
                .into_iter()
                .find(|node| node.id == node_id)
                .ok_or_else(|| AppError::input("目标舰船不存在或已被移出舰队。"))?;
            let lease = connect(&node).await?;
            let exec = lease
                .create_exec(
                    &container_id,
                    CreateExecOptions {
                        attach_stdin: Some(true),
                        attach_stdout: Some(true),
                        attach_stderr: Some(true),
                        tty: Some(true),
                        cmd: Some(command),
                        ..Default::default()
                    },
                )
                .await
                .map_err(AppError::docker)?;
            let (output, mut exec_input) = match lease
                .start_exec(
                    &exec.id,
                    Some(StartExecOptions {
                        detach: false,
                        tty: true,
                        output_capacity: None,
                    }),
                )
                .await
                .map_err(AppError::docker)?
            {
                StartExecResults::Attached { output, input } => (output, input),
                StartExecResults::Detached => {
                    return Err(AppError::terminal("Docker 未返回交互式终端数据流。"));
                }
            };
            if let Err(error) = resize(&lease, &exec.id, cols.0, cols.1).await {
                let _ = exec_input.shutdown().await;
                return Err(error);
            }

            let (input_sender, input_receiver) = mpsc::channel(32);
            let (cancel_sender, cancel_receiver) = oneshot::channel();
            let run_id = Uuid::new_v4().to_string();
            {
                let mut sessions = manager.lock();
                match sessions.get(&session_id) {
                    Some(session)
                        if session.node_id == node_id && session.container_id == container_id =>
                    {
                        return Ok(TerminalOpenResult {
                            session_id,
                            reused: true,
                        });
                    }
                    Some(_) => return Err(AppError::input("终端会话编号已用于其他集装箱。")),
                    None => {}
                }
                sessions.insert(
                    session_id.clone(),
                    TerminalSession {
                        run_id: run_id.clone(),
                        node_id,
                        container_id,
                        lease,
                        exec_id: exec.id,
                        input: input_sender,
                        cancel: Some(cancel_sender),
                    },
                );
            }
            tauri::async_runtime::spawn(run_terminal(
                app.clone(),
                session_id.clone(),
                run_id,
                output,
                exec_input,
                input_receiver,
                cancel_receiver,
            ));
            Ok(TerminalOpenResult {
                session_id,
                reused: false,
            })
        }
        .await,
    )
}

#[tauri::command]
pub async fn terminal_input(app: AppHandle, session_id: String, data: String) -> ApiResponse<bool> {
    let result = async {
        let sender = app
            .state::<TerminalManager>()
            .lock()
            .get(&session_id)
            .map(|session| session.input.clone())
            .ok_or_else(|| AppError::input("终端会话不存在或已经关闭。"))?;
        sender
            .send(data.into_bytes())
            .await
            .map_err(|_| AppError::terminal("终端输入通道已经关闭，请重新打开终端。"))?;
        Ok(true)
    }
    .await;
    respond(result)
}

#[tauri::command]
pub async fn terminal_resize(
    app: AppHandle,
    session_id: String,
    cols: u16,
    rows: u16,
) -> ApiResponse<bool> {
    let result = async {
        valid_size(cols, rows)?;
        let (lease, exec_id) = {
            let manager = app.state::<TerminalManager>();
            let sessions = manager.lock();
            let session = sessions
                .get(&session_id)
                .ok_or_else(|| AppError::input("终端会话不存在或已经关闭。"))?;
            (session.lease.clone(), session.exec_id.clone())
        };
        resize(&lease, &exec_id, cols, rows).await?;
        Ok(true)
    }
    .await;
    respond(result)
}

#[tauri::command]
pub async fn terminal_close(app: AppHandle, session_id: String) -> ApiResponse<bool> {
    respond(close_session(&app, &session_id))
}

fn close_session(app: &AppHandle, session_id: &str) -> Result<bool, AppError> {
    let manager = app.state::<TerminalManager>();
    let session = manager.lock().remove(session_id);
    if let Some(mut session) = session {
        if let Some(cancel) = session.cancel.take() {
            let _ = cancel.send(());
        }
        Ok(true)
    } else {
        Ok(false)
    }
}

fn existing_session(
    manager: &TerminalManager,
    session_id: &str,
    node_id: &str,
    container_id: &str,
) -> Result<Option<TerminalOpenResult>, AppError> {
    let sessions = manager.lock();
    match sessions.get(session_id) {
        Some(session) if session.node_id == node_id && session.container_id == container_id => {
            Ok(Some(TerminalOpenResult {
                session_id: session_id.to_string(),
                reused: true,
            }))
        }
        Some(_) => Err(AppError::input("终端会话编号已用于其他集装箱。")),
        None => Ok(None),
    }
}

fn valid_size(cols: u16, rows: u16) -> Result<(u16, u16), AppError> {
    if cols == 0 || rows == 0 {
        Err(AppError::input("终端宽度和高度必须大于零。"))
    } else {
        Ok((cols, rows))
    }
}

async fn resize(lease: &DockerLease, exec_id: &str, cols: u16, rows: u16) -> Result<(), AppError> {
    lease
        .resize_exec(
            exec_id,
            ResizeExecOptions {
                width: cols,
                height: rows,
            },
        )
        .await
        .map_err(AppError::docker)
}

type ExecOutput = std::pin::Pin<
    Box<dyn futures_util::Stream<Item = Result<LogOutput, bollard::errors::Error>> + Send>,
>;

async fn run_terminal(
    app: AppHandle,
    session_id: String,
    run_id: String,
    mut output: ExecOutput,
    mut exec_input: std::pin::Pin<Box<dyn AsyncWrite + Send>>,
    mut input_receiver: mpsc::Receiver<Vec<u8>>,
    mut cancel: oneshot::Receiver<()>,
) {
    let mut sequence = 0_u64;
    emit_status(&app, &session_id, &mut sequence, "open", None);
    let mut input_open = true;
    let mut failure = None;
    loop {
        tokio::select! {
            biased;
            _ = &mut cancel => break,
            data = input_receiver.recv(), if input_open => {
                match data {
                    Some(data) => {
                        if let Err(error) = exec_input.write_all(&data).await {
                            failure = Some(format!("写入终端标准输入失败：{error}"));
                            break;
                        }
                        if let Err(error) = exec_input.flush().await {
                            failure = Some(format!("发送终端输入失败：{error}"));
                            break;
                        }
                    }
                    None => {
                        input_open = false;
                        let _ = exec_input.shutdown().await;
                    }
                }
            }
            item = output.next() => {
                match item {
                    Some(Ok(output)) => {
                        let event = TerminalOutputEvent {
                            session_id: session_id.clone(),
                            seq: next_sequence(&mut sequence),
                            data: BASE64.encode(output.into_bytes()),
                        };
                        let _ = app.emit("terminal-output", event);
                    }
                    Some(Err(error)) => {
                        failure = Some(format!("读取终端输出失败：{error}"));
                        break;
                    }
                    None => break,
                }
            }
        }
    }
    let _ = exec_input.shutdown().await;
    if !app
        .state::<TerminalManager>()
        .remove_if(&session_id, &run_id)
    {
        return;
    }
    match failure {
        Some(message) => emit_status(&app, &session_id, &mut sequence, "error", Some(message)),
        None => emit_status(&app, &session_id, &mut sequence, "closed", None),
    }
}

fn next_sequence(sequence: &mut u64) -> u64 {
    let current = *sequence;
    *sequence = sequence.saturating_add(1);
    current
}

fn emit_status(
    app: &AppHandle,
    session_id: &str,
    sequence: &mut u64,
    status: &str,
    message: Option<String>,
) {
    let event = TerminalStatusEvent {
        session_id: session_id.to_string(),
        seq: next_sequence(sequence),
        status: status.to_string(),
        message,
    };
    let _ = app.emit("terminal-status", event);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_sequence_is_monotonic() {
        let mut sequence = 0;
        assert_eq!(next_sequence(&mut sequence), 0);
        assert_eq!(next_sequence(&mut sequence), 1);
        assert_eq!(next_sequence(&mut sequence), 2);
    }

    #[test]
    fn invalid_terminal_dimensions_are_rejected() {
        assert!(valid_size(0, 24).is_err());
        assert!(valid_size(80, 0).is_err());
        assert_eq!(valid_size(80, 24).unwrap(), (80, 24));
    }
}
