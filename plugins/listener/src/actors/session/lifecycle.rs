use tauri_specta::Event;

use super::session_span;
use crate::SessionLifecycleEvent;

pub(crate) fn emit_session_ended(
    app: &tauri::AppHandle,
    session_id: &str,
    failure_reason: Option<String>,
) {
    let span = session_span(session_id);
    let _guard = span.enter();

    {
        use tauri_plugin_tray::TrayPluginExt;
        let _ = app.tray().set_start_disabled(false);
    }

    if let Err(error) = (SessionLifecycleEvent::Inactive {
        session_id: session_id.to_string(),
        error: failure_reason.clone(),
    })
    .emit(app)
    {
        tracing::error!(?error, "failed_to_emit_inactive");
    }

    if let Some(reason) = failure_reason {
        tracing::info!(failure_reason = %reason, "session_stopped");
    } else {
        tracing::info!("session_stopped");
    }

}

pub(crate) async fn wait_for_actor_shutdown(actor_name: ractor::ActorName) {
    for _ in 0..50 {
        if ractor::registry::where_is(actor_name.clone()).is_none() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}
