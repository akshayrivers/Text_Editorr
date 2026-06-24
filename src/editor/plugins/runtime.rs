// PluginRuntime owns all plugins and runs on a dedicated OS thread
// with its own tokio runtime. The main thread never blocks on it.
//
// Channels: (Why did I use channels cause hmm it's easier this way. And simple and fucntional over optimised and complex)
//   plugin_tx  (main → runtime)  sends PluginMessage
//   plugin_rx  (runtime → main)  sends PluginResponse
//
// The core polls plugin_rx with try_recv() at the top of each frame.

use super::{Plugin, PluginMessage, PluginResponse};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

/// Handle held by the main thread.
pub struct PluginRuntime {
    /// Send messages to the plugin worker thread.
    pub tx: Sender<PluginMessage>,
    /// Receive responses from the plugin worker thread.
    pub rx: Receiver<PluginResponse>,
}

impl Default for PluginRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginRuntime {
    /// Create a runtime with no plugins yet.
    /// Spawn the worker thread immediately — plugins can be added via `load`.
    pub fn new() -> Self {
        let (core_tx, worker_rx) = mpsc::channel::<PluginMessage>();
        let (worker_tx, core_rx) = mpsc::channel::<PluginResponse>();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build plugin tokio runtime");

            rt.block_on(plugin_worker(worker_rx, worker_tx));
        });

        Self {
            tx: core_tx,
            rx: core_rx,
        }
    }

    /// Send a message to the plugin runtime (fire and forget).
    pub fn send(&self, msg: PluginMessage) {
        // Ignore send errors — worker may have already shut down cleanly.
        let _ = self.tx.send(msg);
    }

    /// Poll all pending responses without blocking.
    /// Returns all responses that arrived since last call.
    pub fn drain_responses(&self) -> Vec<PluginResponse> {
        let mut responses = Vec::new();
        loop {
            match self.rx.try_recv() {
                Ok(response) => responses.push(response),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => break,
            }
        }
        responses
    }

    /// Register a plugin. Sends a LoadPlugin message to the worker.
    pub fn load_plugin(&self, plugin: Box<dyn Plugin>) {
        let _ = self.tx.send(PluginMessage::LoadPlugin(plugin));
    }

    /// Graceful shutdown.
    pub fn shutdown(&self) {
        let _ = self.tx.send(PluginMessage::Shutdown);
    }
}

// Worker

async fn plugin_worker(rx: std::sync::mpsc::Receiver<PluginMessage>, tx: Sender<PluginResponse>) {
    let mut plugins: Vec<Box<dyn Plugin>> = Vec::new();

    loop {
        // Block until a message arrives (this is the async worker — it's
        // fine to block here since it runs on its own thread).
        let msg = match rx.recv() {
            Ok(m) => m,
            Err(_) => break, // channel closed
        };

        match msg {
            PluginMessage::LoadPlugin(mut plugin) => {
                plugin.on_load().await;
                plugins.push(plugin);
            }

            PluginMessage::Event(event) => {
                for plugin in &mut plugins {
                    if let Some(response) = plugin.on_event(&event).await {
                        let _ = tx.send(response);
                    }
                }
            }

            PluginMessage::BufferChanged(snapshot) => {
                for plugin in &mut plugins {
                    if let Some(response) = plugin.on_buffer_change(snapshot.clone()).await {
                        let _ = tx.send(response);
                    }
                }
            }

            PluginMessage::Shutdown => {
                for plugin in &mut plugins {
                    plugin.on_unload().await;
                }
                break;
            }
        }
    }
}
