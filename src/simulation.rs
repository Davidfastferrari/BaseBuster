use crate::events::Event;
use log::info;
use tokio::sync::broadcast::{Receiver, Sender};

pub async fn simulate_path(sim_sender: Sender<Event>, mut opt_receiver: Receiver<Event>) {
    while let Ok(Event::OptimizedPath(opt_path)) = opt_receiver.recv().await {
        info!("Got a optimal path");
    }
}
