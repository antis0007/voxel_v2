//! Live Steam/SDR prerequisite probe; does not claim host/join gameplay.
use std::time::{Duration,Instant};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let _scope=engine_net::steam::RuntimeScope;
    let sdk=engine_net::steam::runtime()?;
    let start=Instant::now();let mut previous=String::new();
    while start.elapsed()<Duration::from_secs(20) {
        sdk.run_callbacks();
        let status=engine_net::steam::relay_status()?;
        if status!=previous{println!("[steamworks-probe] {status}");previous=status;}
        std::thread::sleep(Duration::from_millis(50));
    }
    println!("[steamworks-probe] complete; gameplay WAN remains a separate check");Ok(())
}
