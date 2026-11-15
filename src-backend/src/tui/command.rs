use super::Tab;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ShipCommand {
    SwitchTab(Tab),
    Start,
    Stop,
    Restart,
    Logs,
    Remove,
    Refresh,
    Help,
    Update,
    Quit,
}

pub(super) fn parse_command(input: &str) -> Result<Option<ShipCommand>, String> {
    let mut parts = input.split_whitespace();
    let Some(name) = parts.next() else {
        return Ok(None);
    };
    let command = match name.to_ascii_lowercase().as_str() {
        "1" => ShipCommand::SwitchTab(Tab::Containers),
        "2" => ShipCommand::SwitchTab(Tab::Images),
        "3" => ShipCommand::SwitchTab(Tab::Compose),
        "4" => ShipCommand::SwitchTab(Tab::Nodes),
        "5" => ShipCommand::SwitchTab(Tab::Registries),
        "6" => ShipCommand::SwitchTab(Tab::Help),
        "start" => ShipCommand::Start,
        "stop" => ShipCommand::Stop,
        "restart" => ShipCommand::Restart,
        "logs" => ShipCommand::Logs,
        "rm" => ShipCommand::Remove,
        "refresh" => ShipCommand::Refresh,
        "help" => ShipCommand::Help,
        "update" => ShipCommand::Update,
        "quit" => ShipCommand::Quit,
        _ => return Err(format!("未知航令“{name}”，输入 help 查看可用指令")),
    };
    if parts.next().is_some() {
        return Err(format!("航令 {name} 不接受参数"));
    }
    Ok(Some(command))
}

#[cfg(test)]
mod tests {
    use super::{parse_command, ShipCommand};
    use crate::tui::Tab;

    #[test]
    fn parses_tab_commands() {
        let cases = [
            ("1", Tab::Containers),
            ("2", Tab::Images),
            ("3", Tab::Compose),
            ("4", Tab::Nodes),
            ("5", Tab::Registries),
            ("6", Tab::Help),
        ];
        for (input, tab) in cases {
            assert_eq!(parse_command(input), Ok(Some(ShipCommand::SwitchTab(tab))));
        }
    }

    #[test]
    fn parses_action_commands_and_trims_input() {
        let cases = [
            ("start", ShipCommand::Start),
            ("stop", ShipCommand::Stop),
            ("restart", ShipCommand::Restart),
            ("logs", ShipCommand::Logs),
            ("rm", ShipCommand::Remove),
            ("refresh", ShipCommand::Refresh),
            ("help", ShipCommand::Help),
            ("update", ShipCommand::Update),
            ("quit", ShipCommand::Quit),
            ("  START  ", ShipCommand::Start),
        ];
        for (input, command) in cases {
            assert_eq!(parse_command(input), Ok(Some(command)));
        }
    }

    #[test]
    fn accepts_empty_command_without_action() {
        assert_eq!(parse_command(""), Ok(None));
        assert_eq!(parse_command(" \t  "), Ok(None));
    }

    #[test]
    fn rejects_arguments_and_unknown_commands() {
        for input in ["start now", "1 2", "quit force"] {
            assert!(parse_command(input).unwrap_err().contains("不接受参数"));
        }
        for input in ["docker ps", "sh", "unknown"] {
            assert!(parse_command(input).unwrap_err().contains("未知航令"));
        }
    }
}
