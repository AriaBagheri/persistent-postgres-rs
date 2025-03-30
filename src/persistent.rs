use crate::error::connection::PersistentPostgresConnectionError;
use crate::error::listener::PersistentPostgresListenerError;
use colored::Colorize;
use futures_util::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::PgListener;
use sqlx::{Pool, Postgres};
use std::collections::HashMap;
use std::sync::LazyLock;
use std::time::Duration;
use standard_error::traits::StandardErrorDescriptionTrait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::broadcast::{Receiver, Sender};
use tokio::sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard};
use tokio::task::JoinHandle;

/// # PersistentPostgres
///
/// `PersistentPostgres` is an asynchronous manager for PostgreSQL connections that
/// encapsulates a connection pool, optional notification listener, and background tasks
/// for monitoring and processing PostgreSQL notifications. It supports dynamically
/// listening/unlistening on channels and registering callback functions to handle
/// notifications.
pub struct PersistentPostgres {
    address: RwLock<Option<String>>,
    pool: RwLock<Option<Pool<Postgres>>>,
    with_listener: bool,
    listener: Mutex<Option<PgListener>>,
    channels: Mutex<Vec<String>>,
    notifications_handle: Mutex<Option<JoinHandle<()>>>,
    monitor_handle: Mutex<Option<JoinHandle<()>>>,
    // HashMap on channel -> tag -> listener callback function.
    listeners: LazyLock<
        RwLock<
            HashMap<
                String,
                HashMap<String, fn(PgChangeNotification, Receiver<()>) -> JoinHandle<()>>,
            >,
        >,
    >,
    join_handles: Mutex<Vec<(Sender<()>, JoinHandle<()>)>>,
    shutdown_signal_channel: LazyLock<Sender<()>>,
}

impl PersistentPostgres {
    /// Constructs a new `PersistentPostgres` instance.
    ///
    /// The `with_listener` flag determines whether a [`PgListener`] will be established.
    pub const fn default(with_listener: bool) -> Self {
        PersistentPostgres {
            address: RwLock::const_new(None),
            pool: RwLock::const_new(None),
            with_listener,
            listener: Mutex::const_new(None),
            channels: Mutex::const_new(Vec::new()),
            notifications_handle: Mutex::const_new(None),
            monitor_handle: Mutex::const_new(None),
            listeners: LazyLock::new(|| RwLock::const_new(HashMap::new())),
            join_handles: Mutex::const_new(Vec::new()),
            shutdown_signal_channel: LazyLock::new(|| Sender::new(2)),
        }
    }

    /// Initiates the background monitoring and notification processing tasks.
    ///
    /// This method spawns:
    /// - A monitoring thread to check the health of the connection pool.
    /// - A notifications thread to receive and dispatch PostgreSQL notifications.
    pub async fn initiate(&'static self) {
        *self.monitor_handle.lock().await = Some(self.monitor_thread());
        *self.notifications_handle.lock().await = Some(self.notifications_thread());
    }

    pub async fn pool(&self) -> RwLockReadGuard<'_, Pool<Postgres>> {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            let x = self.pool.read().await;
            if x.is_some() {
                return RwLockReadGuard::map(x, |f| f.as_ref().unwrap());
            }
            drop(x);
            interval.tick().await;
        }
    }

    /// Sets the PostgreSQL connection URI and establishes a connection pool.
    ///
    /// Optionally, if `with_listener` is enabled, also establishes a [`PgListener`]
    /// and listens on any pre-registered channels.
    pub async fn set_uri(&self, uri: &str) -> Result<(), PersistentPostgresConnectionError> {
        if let Some(old_address) = self.address.read().await.as_ref() {
            if old_address.as_str() == uri {
                println!(
                    "{}",
                    "POSTGRES - SET_ADDRESS - Address unchanged. \
                    Skipping pool re-establishment."
                        .green()
                        .dimmed()
                );
                return Ok(());
            }
        }
        let (pool, listener) =
            Self::establish_pool(uri, self.with_listener, self.channels.lock().await).await?;

        *self.address.write().await = Some(uri.to_string());
        *self.pool.write().await = Some(pool);
        *self.listener.lock().await = listener;

        Ok(())
    }

    /// Establishes a PostgreSQL connection pool and optionally a notification listener.
    ///
    /// If `with_listener` is true, this method will attempt to connect a [`PgListener`] with
    /// the established pool and set it to listen on all channels provided.
    pub async fn establish_pool<'a>(
        uri: &str,
        with_listener: bool,
        channels: MutexGuard<'a, Vec<String>>,
    ) -> Result<(Pool<Postgres>, Option<PgListener>), PersistentPostgresConnectionError> {
        println!(
            "{}",
            "POSTGRES - PG_POOL - Establishing connection pool..."
                .to_string()
                .blue()
        );
        let pool = match Pool::connect(uri).await {
            Ok(pool) => {
                println!(
                    "{}",
                    "POSTGRES - PG_POOL - Connection pool established successfully!".green()
                );
                Ok(pool)
            }
            Err(e) => {
                println!(
                    "{}",
                    format!(
                        "POSTGRES - PG_POOL - {} - {}",
                        PersistentPostgresConnectionError::FailedToEstablishConnection
                            .description()
                            .unwrap_or_default(),
                        e.to_string()
                    )
                        .red()
                );
                Err(PersistentPostgresConnectionError::FailedToEstablishConnection)
            }
        }?;
        if with_listener {
            println!(
                "{}",
                "POSTGRES - PG_LISTENER - Establishing listener...".blue()
            );
            match PgListener::connect_with(&pool).await {
                Ok(mut listener) => {
                    println!(
                        "{}",
                        "POSTGRES - PG_LISTENER - Listener established successfully!".green()
                    );
                    for channel in channels.iter() {
                        match listener.listen(channel).await {
                            Ok(_) => {
                                println!(
                                    "{}",
                                    format!(
                                        "POSTGRES - PG_LISTENER - Listener listening on {channel}!"
                                    )
                                        .green()
                                );
                            }
                            Err(e) => {
                                println!(
                                    "{} {}",
                                    "POSTGRES - PG_LISTENER - Failed to listen to channel. Soft-fail. Moving on".red(),
                                    format!(
                                        "| channel = {}, error = {}",
                                        channel,
                                        e.to_string()
                                    )
                                        .red()
                                        .dimmed()
                                );
                            }
                        }
                    }
                    Ok((pool, Some(listener)))
                }
                Err(e) => {
                    println!(
                        "{}",
                        format!(
                            "POSTGRES - PG_LISTENER - {} - {}",
                            PersistentPostgresConnectionError::FailedToEstablishListener
                                .description()
                                .unwrap_or_default(),
                            e.to_string()
                        )
                            .red()
                    );
                    Err(PersistentPostgresConnectionError::FailedToEstablishListener)
                }
            }
        } else {
            Ok((pool, None))
        }
    }

    /// Spawns a background task that monitors the connection pool's health.
    ///
    /// This monitoring thread periodically checks if the connection pool is closed.
    /// If a loss is detected, it attempts to re-establish the pool (and listener if enabled)
    /// using the stored URI.
    pub fn monitor_thread(&'static self) -> JoinHandle<()> {
        let mut shutdown = self.shutdown_signal_channel.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_millis(250));
            loop {
                tokio::select! {
                    _ = shutdown.recv() => break,
                    _ = interval.tick() => {
                        let mut client_lost = false;
                        {
                            let pool = self.pool.read().await;
                            if let Some(pool) = pool.as_ref() {
                                if pool.is_closed() {
                                    client_lost = true;
                                }
                            } else {
                                client_lost = true;
                            }
                        }
                        if client_lost {
                            println!("{}", "POSTGRES - MONITORING - Connection pool loss detected! Re-establishing connection pool...".to_string().yellow());
                            if let Some(address) = self.address.read().await.as_ref() {
                                match Self::establish_pool(address, self.with_listener, self.channels.lock().await).await {
                                    Ok((pool, listener)) => {
                                        *self.pool.write().await = Some(pool);
                                        *self.listener.lock().await = listener;
                                    }
                                    Err(_) => {
                                        println!("{}", "POSTGRES - MONITORING - Could not establish connection pool. Retrying in 250ms...".to_string().red());
                                    }
                                }
                            }
                        }
                    }
                }
                // Note: The listener is not checked separately since it uses the same pool.
            }
        })
    }

    /// Spawns a background task to process PostgreSQL notifications.
    ///
    /// This task continuously waits for notifications from the [`PgListener`]. When a notification
    /// is received, it attempts to deserialize it into a [`PgChangeNotification`] and then dispatches
    /// it to all registered listener callbacks for the corresponding channel.
    pub fn notifications_thread(&'static self) -> JoinHandle<()> {
        let mut shutdown = self.shutdown_signal_channel.subscribe();
        let mut interval = tokio::time::interval(Duration::from_millis(10));
        tokio::spawn(async move {
            loop {
                if let Some(listener) = self.listener.lock().await.as_mut() {
                    tokio::select! {
                        _ = shutdown.recv() => break,
                        _ = interval.tick() => continue,
                        Ok(notification) = listener.recv() => {
                            if let Ok(val) = serde_json::from_str::<PgChangeNotification>(notification.payload()) {
                                let listeners = self.listeners.read().await;
                                if let Some(callbacks) = listeners.get(notification.channel()).map(|m| m.values()) {
                                    for listener_function in callbacks {
                                        let sender = Sender::new(1);
                                        let handle = listener_function(val.clone(), sender.subscribe());
                                        self.join_handles.lock().await.push((sender, handle));
                                    }
                                }
                            }
                        }
                    }
                } else {
                    tokio::select! {
                        _ = shutdown.recv() => break,
                        _ = interval.tick() => {continue}
                    }
                }
            }
        })
    }

    /// Adds a new channel to listen for PostgreSQL notifications.
    ///
    /// If a [`PgListener`] is already established, it immediately listens on the provided channel.
    /// The channel is also added to the internal list for future reconnections.
    pub async fn listen(&self, channel: &str) -> Result<(), PersistentPostgresListenerError> {
        self.channels.lock().await.push(channel.to_string());
        if let Some(listener) = self.listener.lock().await.as_mut() {
            return listener.listen(&channel).await.map_err(|e| {
                println!(
                    "{} {}",
                    "POSTGRES - LISTEN - Failed to listen to channel".red(),
                    format!("| channel = {}, error = {}", channel, e.to_string())
                        .red()
                        .dimmed()
                );
                PersistentPostgresListenerError::FailedToAddChannelListener
            });
        } else {
            println!(
                "{} {}",
                "POSTGRES - LISTEN - No pg listener yet. Will add channel when one establishes!"
                    .yellow(),
                format!("| channel = {}", channel).yellow().dimmed()
            );
        }
        Ok(())
    }

    /// Removes a channel from listening for PostgreSQL notifications.
    ///
    /// Attempts to stop listening on the provided channel using the [`PgListener`].
    /// Also removes the channel from the internal list.
    pub async fn unlisten(&self, channel: &str) -> Result<(), PersistentPostgresListenerError> {
        if let Some(listener) = self.listener.lock().await.as_mut() {
            return match listener.unlisten(&channel).await {
                Ok(_) => {
                    let mut channels = self.channels.lock().await;
                    *channels = channels
                        .iter()
                        .filter(|c| c.as_str() == channel)
                        .cloned()
                        .collect();
                    println!(
                        "{} {}",
                        "POSTGRES - UNLISTEN - Unlisten to channel successful".green(),
                        format!("| channel = {}", channel).green().dimmed()
                    );
                    Ok(())
                }
                Err(e) => {
                    println!(
                        "{} {}",
                        "POSTGRES - UNLISTEN - Failed to unlisten to channel".red(),
                        format!("| channel = {}, error = {}", channel, e.to_string())
                            .red()
                            .dimmed()
                    );
                    Err(PersistentPostgresListenerError::FailedToAddChannelListener)
                }
            };
        } else {
            let mut channels = self.channels.lock().await;
            *channels = channels
                .iter()
                .filter(|c| c.as_str() == channel)
                .cloned()
                .collect();
            println!(
                "{} {}",
                "POSTGRES - UNLISTEN - No pg listener found. So we failed successfully!".yellow(),
                format!("| channel = {}", channel).yellow().dimmed()
            );
        }
        Ok(())
    }

    /// Registers a notification listener callback for a specified channel.
    ///
    /// The callback function should accept a [`PgChangeNotification`] and return a [`JoinHandle<()>`]
    /// for the spawned task that will process the notification.
    pub async fn add_listener(
        &self,
        channel: String,
        tag: String,
        function: fn(PgChangeNotification, Receiver<()>) -> JoinHandle<()>,
    ) {
        let mut all_listeners = self.listeners.write().await;
        let channel_listeners = all_listeners.entry(channel).or_insert_with(HashMap::new);
        channel_listeners.insert(tag, function);
    }

    /// Removes a registered notification listener callback for a given channel and tag.
    pub async fn remove_listener(&self, channel: String, tag: String) {
        let mut all_listeners = self.listeners.write().await;
        all_listeners.entry(channel).and_modify(|map| {
            map.remove(&tag);
        });
    }

    /// Gracefully shuts down the PostgreSQL connection pool, listener, and background tasks.
    ///
    /// This method closes the connection pool, drops the listener, and aborts all background tasks,
    /// including the monitoring and notification threads as well as any individual listener handlers.
    pub async fn shutdown(&self) {
        print!("\n");

        let mut join_handles = self.join_handles.lock().await;
        let join_handles_len = join_handles.len();
        for (i, (sender, handle)) in join_handles.iter_mut().enumerate() {
            let _ = sender.send(());
            match tokio::time::timeout(Duration::from_secs(5), &mut *handle).await {
                Ok(_) => {
                    println!(
                        "{} {} {}",
                        "POSTGRES - SHUTDOWN - Listener notification handler killed gracefully"
                            .cyan(),
                        format!("({}/{})", i + 1, join_handles_len).cyan(),
                        "...".cyan()
                    );
                }
                Err(_) => {
                    println!(
                        "{} {} {} {}",
                        "POSTGRES - SHUTDOWN - Listener notification handler killed".cyan(),
                        "forcefully".red(),
                        format!("({}/{})", i + 1, join_handles_len).cyan(),
                        "...".cyan()
                    );
                    handle.abort();
                }
            }
        }
        drop(join_handles);

        let _ = self.shutdown_signal_channel.send(());
        println!(
            "{}",
            "POSTGRES - SHUTDOWN - Shutdown signal was propagated to internal threads!".cyan()
        );

        if let Some(monitor_thread) = self.monitor_handle.lock().await.as_mut() {
            let _ = tokio::time::timeout(Duration::from_secs(5), &mut *monitor_thread)
                .await
                .map(|_| {
                    println!(
                        "{}",
                        "POSTGRES - SHUTDOWN - Monitoring thread terminated gracefully!".cyan()
                    );
                })
                .map_err(|_| {
                    println!(
                        "{} {}",
                        "POSTGRES - SHUTDOWN - Monitoring thread terminated".cyan(),
                        "forcefully".bold().red()
                    );
                    monitor_thread.abort();
                });
        }
        if let Some(notifications_handle) = self.notifications_handle.lock().await.as_mut() {
            let _ = tokio::time::timeout(Duration::from_secs(5), &mut *notifications_handle)
                .await
                .map(|_| {
                    println!(
                        "{}",
                        "POSTGRES - SHUTDOWN - Notifications thread terminated gracefully!".cyan()
                    );
                })
                .map_err(|_| {
                    println!(
                        "{} {}",
                        "POSTGRES - SHUTDOWN - Notifications thread terminated".cyan(),
                        "forcefully".bold().red()
                    );
                    notifications_handle.abort();
                });
        }

        if let Some(listener) = self.listener.lock().await.take() {
            drop(listener);
            println!(
                "{}",
                "POSTGRES - SHUTDOWN - Postgres listener dropped gracefully!".cyan()
            );
        }

        if let Some(connection) = self.pool.write().await.as_mut() {
            match tokio::time::timeout(Duration::from_secs(5), connection.close()).await {
                Ok(_) => {
                    println!(
                        "{}",
                        "POSTGRES - SHUTDOWN - Postgres connection closed gracefully!".cyan()
                    );
                },
                Err(_) => {
                    println!(
                        "{} {}",
                        "POSTGRES - SHUTDOWN - Postgres connection terminated".cyan(),
                        "forcefully".bold().red()
                    );
                }
            }
        }

        println!("{}", "POSTGRES - SHUTDOWN - Goodbye!".cyan())
    }
}

/// A global static instance of `PersistentPostgres` that can be used throughout the application.
/// In this example, the listener is disabled (i.e. `with_listener` is set to `false`).
pub static POSTGRES: PersistentPostgres = PersistentPostgres::default(false);

/// # PgChangeAction
///
/// Represents the type of change notification received from PostgreSQL.
/// The variants indicate whether the change was an update, delete, or insert.
#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(rename_all = "UPPERCASE")]
pub enum PgChangeAction {
    Update,
    Delete,
    Insert,
}

/// # PgChangeNotification
///
/// A notification structure containing details about a change in the PostgreSQL database.
/// - `action`: The type of change (insert, update, or delete).
/// - `payload`: The JSON payload containing the details of the change.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PgChangeNotification {
    pub action: PgChangeAction,
    pub payload: Value,
}
