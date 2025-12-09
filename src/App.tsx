import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";

interface StravaAuthStatus {
  is_authenticated: boolean;
  expires_at: string | null;
  needs_refresh: boolean;
}

interface SyncResult {
  new_activities: number;
  total_fetched: number;
}

interface Workout {
  id: number;
  strava_id: string;
  activity_type: string;
  started_at: string;
  duration_seconds: number | null;
  distance_meters: number | null;
  elevation_gain_meters: number | null;
  average_heartrate: number | null;
  max_heartrate: number | null;
  average_watts: number | null;
  suffer_score: number | null;
}

function App() {
  const [stravaStatus, setStravaStatus] = useState<StravaAuthStatus | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [isSyncing, setIsSyncing] = useState(false);
  const [syncResult, setSyncResult] = useState<SyncResult | null>(null);
  const [workouts, setWorkouts] = useState<Workout[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    checkStravaStatus();
    loadWorkouts();
  }, []);

  async function checkStravaStatus() {
    try {
      const status = await invoke<StravaAuthStatus>("strava_get_auth_status");
      setStravaStatus(status);
      setError(null);
    } catch (e) {
      setError(`Failed to check status: ${e}`);
    }
  }

  async function loadWorkouts() {
    try {
      const data = await invoke<Workout[]>("get_workouts");
      setWorkouts(data);
    } catch (e) {
      console.error("Failed to load workouts:", e);
    }
  }

  async function connectStrava() {
    setIsConnecting(true);
    setError(null);

    try {
      const authUrl = await invoke<string>("strava_start_auth");
      await openUrl(authUrl);
      await invoke("strava_complete_auth");
      await checkStravaStatus();
    } catch (e) {
      setError(`Connection failed: ${e}`);
    } finally {
      setIsConnecting(false);
    }
  }

  async function disconnectStrava() {
    try {
      await invoke("strava_disconnect");
      await checkStravaStatus();
    } catch (e) {
      setError(`Disconnect failed: ${e}`);
    }
  }

  async function syncActivities() {
    setIsSyncing(true);
    setError(null);
    setSyncResult(null);

    try {
      const result = await invoke<SyncResult>("strava_sync_activities");
      setSyncResult(result);
      await loadWorkouts();
    } catch (e) {
      setError(`Sync failed: ${e}`);
    } finally {
      setIsSyncing(false);
    }
  }

  function formatDuration(seconds: number | null): string {
    if (!seconds) return "-";
    const hours = Math.floor(seconds / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);
    if (hours > 0) {
      return `${hours}h ${minutes}m`;
    }
    return `${minutes}m`;
  }

  function formatDistance(meters: number | null): string {
    if (!meters) return "-";
    const km = meters / 1000;
    return `${km.toFixed(1)} km`;
  }

  return (
    <main className="container">
      <h1>Trainer Log</h1>
      <p className="subtitle">Ambient training coach</p>

      <div className="card">
        <h2>Strava Connection</h2>

        {stravaStatus === null ? (
          <p>Loading...</p>
        ) : stravaStatus.is_authenticated ? (
          <div>
            <p className="status connected">Connected</p>
            <div className="button-row">
              <button onClick={syncActivities} disabled={isSyncing}>
                {isSyncing ? "Syncing..." : "Sync Activities"}
              </button>
              <button onClick={disconnectStrava} className="secondary">
                Disconnect
              </button>
            </div>
            {syncResult && (
              <p className="sync-result">
                Synced {syncResult.new_activities} new activities
                {syncResult.total_fetched > 0 && ` (${syncResult.total_fetched} checked)`}
              </p>
            )}
          </div>
        ) : (
          <div>
            <p className="status disconnected">Not connected</p>
            <button onClick={connectStrava} disabled={isConnecting}>
              {isConnecting ? "Connecting..." : "Connect Strava"}
            </button>
          </div>
        )}

        {error && <p className="error">{error}</p>}
      </div>

      {workouts.length > 0 && (
        <div className="card">
          <h2>Recent Workouts</h2>
          <div className="workout-list">
            {workouts.slice(0, 10).map((workout) => (
              <div key={workout.id} className="workout-item">
                <div className="workout-header">
                  <span className="workout-type">{workout.activity_type}</span>
                  <span className="workout-date">
                    {new Date(workout.started_at).toLocaleDateString()}
                  </span>
                </div>
                <div className="workout-stats">
                  <span>{formatDuration(workout.duration_seconds)}</span>
                  <span>{formatDistance(workout.distance_meters)}</span>
                  {workout.average_heartrate && (
                    <span>{workout.average_heartrate} bpm</span>
                  )}
                  {workout.suffer_score && (
                    <span className="suffer-score">{workout.suffer_score} effort</span>
                  )}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </main>
  );
}

export default App;
