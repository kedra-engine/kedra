//! Startup contracts through production editor composition, without native rendering.
#![allow(clippy::unwrap_used, reason = "Headless contract assertions")]
mod bindings;
mod lifecycle;
mod loading;
use crate::test_support::*;
use bevy::{input_focus::InputFocus, prelude::*};
use kedra_scene::{assets::PrefabSources, document::PrefabDocument};
use std::time::{Duration, Instant};
