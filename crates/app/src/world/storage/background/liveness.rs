//! Synthetic acknowledgements exercise the production worker scheduler.
use super::*;

impl SaveService {
    pub(crate) fn check_maintenance_liveness(w: &mut World, player: &Player, day: &DayNightCycle) -> Result<serde_json::Value> {
        let enabled = autosave_enabled();
        set_autosave_enabled(false);
        let result = (|| -> Result<serde_json::Value> {
            let jobs_before = crate::world::farming::sidecar::encode(w);
            let mut cases = Vec::new();
            for kind in ["saved", "collected", "collect_error", "saved_pending"] {
                let (tx, work) = mpsc::sync_channel(1);
                let (done, rx) = mpsc::sync_channel(1);
                let now = Instant::now();
                let mut service = Self {
                    tx: Some(tx), rx, active: true, pending: kind == "saved_pending",
                    manual: kind == "saved_pending", publish_slot: true, applying: None,
                    session: Some((*w.regions.view.root).clone()),
                    next_due: now + Duration::from_secs(60), retry_capture: now,
                    next_collect: now - Duration::from_secs(2), status: None, failed: false,
                };
                let ack = match kind {
                    "collected" => Finished::Collected(Ok(Default::default())),
                    "collect_error" => Finished::Collected(Err(anyhow::anyhow!("Synthetic collection failure"))),
                    _ => Finished::Saved(Ok(Completed {
                        instance_id: w.regions.instance_id, view: w.regions.view.clone(),
                        changed: VecDeque::new(), message: "World saved".into(),
                    })),
                };
                done.send(ack).map_err(|_| anyhow::anyhow!("Synthetic acknowledgement channel closed"))?;
                service.tick(Some(w), player, day);
                let queued = match work.try_recv() {
                    Ok(Work::Save(_)) => "save", Ok(Work::Collect(_)) => "collect", Err(_) => "none",
                };
                let idle_after_ack = !service.busy();
                let pending_retained = kind != "saved_pending" || (service.busy() && (service.pending || queued == "save"));
                let backoff_retained = kind != "collect_error" || service.next_collect >= now + Duration::from_secs(30);
                let first_pass = if kind == "saved_pending" {pending_retained && queued != "collect"}
                    else {idle_after_ack && queued == "none" && backoff_retained};
                let maintenance_later = if matches!(kind, "saved" | "collected") {
                    service.active = false;
                    service.next_collect = Instant::now() - Duration::from_millis(1);
                    service.tick(Some(w), player, day);
                    service.busy() && matches!(work.try_recv(), Ok(Work::Collect(_)))
                } else {true};
                cases.push(serde_json::json!({"case":kind,"queued_on_ack":queued,
                    "idle_after_ack":idle_after_ack,"pending_request_retained":pending_retained,
                    "error_backoff_retained":backoff_retained,"maintenance_runs_when_due":maintenance_later,
                    "passed":first_pass && maintenance_later}));
            }
            ensure!(crate::world::farming::sidecar::encode(w) == jobs_before, "Scheduler probe changed farming state");
            Ok(serde_json::json!({"passed":cases.iter().all(|v|v["passed"]==true),"cases":cases,
                "scope":"Actual SaveService::tick with synthetic completion channels and a disposable loaded world; no persistence or wall-time performance claim"}))
        })();
        set_autosave_enabled(enabled);
        result
    }
}
