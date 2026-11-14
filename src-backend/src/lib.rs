mod compose;
pub mod config;
mod credential;
mod docker;
mod error;
mod ssh;
pub mod template;
mod terminal;
mod utils;

use tauri::{Manager, RunEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(terminal::TerminalManager::default())
        .invoke_handler(tauri::generate_handler![
            config::commands::get_fleet_state,
            config::commands::create_node,
            config::commands::update_node,
            config::commands::remove_node,
            config::commands::set_current_node,
            config::commands::test_node,
            terminal::terminal_open,
            terminal::terminal_input,
            terminal::terminal_resize,
            terminal::terminal_close,
            config::commands::list_registries,
            config::commands::save_registry,
            config::commands::remove_registry,
            config::commands::set_default_registry,
            docker::container::list_containers,
            docker::container::create_container,
            docker::container::start_container,
            docker::container::stop_container,
            docker::container::restart_container,
            docker::container::remove_container,
            docker::container::get_container_logs,
            docker::container::exec_container,
            docker::container::get_fleet_overview,
            docker::image::list_images,
            docker::image::search_images,
            docker::image::pull_image,
            docker::image::remove_image,
            docker::image::export_image,
            docker::image::import_image,
            compose::list_compose_projects,
            compose::save_compose_project,
            compose::remove_compose_project,
            compose::validate_compose,
            compose::compose_up,
            compose::compose_down,
            compose::compose_list,
            compose::compose_logs,
            template::list_compose_templates,
            template::save_compose_template,
            template::remove_compose_template,
        ])
        .build(tauri::generate_context!())
        .expect("ShipWiz 启动失败")
        .run(|app_handle, event| {
            if let RunEvent::ExitRequested { .. } = event {
                app_handle.state::<terminal::TerminalManager>().close_all();
                docker::shutdown_connections();
            }
        });
}
