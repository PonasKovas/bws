// pub mod lobby;
pub mod login;

use crate::chat_parse;
use crate::datatypes::*;
use crate::global_state;
use crate::global_state::{Player, PlayerStream};
use crate::internal_communication::{SHInputSender, SHOutputReceiver, WBound, WReceiver, WSender};
use crate::packets::{ClientBound, ServerBound, TitleAction};
use crate::GLOBAL_STATE;
use anyhow::Result;
use futures::future::FutureExt;
use log::{debug, error, info, warn};
use serde_json::{json, to_string};
use std::{
    thread::Builder,
    time::{Duration, Instant},
};
use tokio::sync::mpsc::error::SendError;
use tokio::{
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::unconstrained,
};
