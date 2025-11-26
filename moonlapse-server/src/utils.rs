use std::fmt::Debug;

use tokio::sync::mpsc;

pub type TxRx<TMsg : Debug + Clone + Copy> = (mpsc::Sender<TMsg>, mpsc::Receiver<TMsg>);
