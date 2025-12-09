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

interface ComputeResult {
  total: number;
  computed: number;
}

interface UserSettings {
  max_hr: number | null;
  lthr: number | null;
  ftp: number | null;
  training_days_per_week: number;
}

interface WorkoutWithMetrics {
  id: number;
  strava_id: string;
  activity_type: string;
  started_at: string;
  duration_seconds: number | null;
  distance_meters: number | null;
  average_heartrate: number | null;
  average_watts: number | null;
  suffer_score: number | null;
  // Computed metrics
  pace_min_per_km: number | null;
  speed_kmh: number | null;
  kj: number | null;
  rtss: number | null;
  efficiency: number | null;
  cardiac_cost: number | null;
  hr_zone: string | null;
}

function App() {
  const [stravaStatus, setStravaStatus] = useState<StravaAuthStatus | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [isSyncing, setIsSyncing] = useState(false);
  const [isComputing, setIsComputing] = useState(false);
  const [syncResult, setSyncResult] = useState<SyncResult | null>(null);
  const [computeResult, setComputeResult] = useState<ComputeResult | null>(null);
  const [workouts, setWorkouts] = useState<WorkoutWithMetrics[]>([]);
  const [settings, setSettings] = useState<UserSettings | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);

  // Form state for settings
  const [maxHrInput, setMaxHrInput] = useState("");
  const [lthrInput, setLthrInput] = useState("");

  useEffect(() => {
    checkStravaStatus();
    loadWorkouts();
    loadSettings();
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
      const data = await invoke<WorkoutWithMetrics[]>("get_workouts_with_metrics", { limit: 50 });
      setWorkouts(data);
    } catch (e) {
      console.error("Failed to load workouts:", e);
    }
  }

  async function loadSettings() {
    try {
      const data = await invoke<UserSettings>("get_user_settings");
      setSettings(data);
      setMaxHrInput(data.max_hr?.toString() || "");
      setLthrInput(data.lthr?.toString() || "");
    } catch (e) {
      console.error("Failed to load settings:", e);
    }
  }

  async function saveSettings() {
    try {
      await invoke("update_user_settings", {
        maxHr: maxHrInput ? parseInt(maxHrInput) : null,
        lthr: lthrInput ? parseInt(lthrInput) : null,
      });
      await loadSettings();
      setShowSettings(false);
    } catch (e) {
      setError(`Failed to save settings: ${e}`);
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

  async function computeMetrics() {
    setIsComputing(true);
    setError(null);
    setComputeResult(null);

    try {
      const result = await invoke<ComputeResult>("compute_workout_metrics");
      setComputeResult(result);
      await loadWorkouts();
    } catch (e) {
      setError(`Compute failed: ${e}`);
    } finally {
      setIsComputing(false);
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

  function formatPace(paceMinPerKm: number | null): string {
    if (!paceMinPerKm) return "-";
    const mins = Math.floor(paceMinPerKm);
    const secs = Math.round((paceMinPerKm - mins) * 60);
    return `${mins}:${secs.toString().padStart(2, "0")}/km`;
  }

  function formatRtss(rtss: number | null): string {
    if (!rtss) return "-";
    return rtss.toFixed(0);
  }

  // Check if any workouts need metrics computed
  const needsCompute = workouts.some(w => w.rtss === null && w.average_heartrate !== null);
  const hasSettings = settings?.max_hr !== null;

  return (
    <main className="container">
      <h1>Trainer Log</h1>
      <p className="subtitle">Ambient training coach</p>

      {/* Settings Card */}
      <div className="card">
        <div className="card-header">
          <h2>Settings</h2>
          <button className="small" onClick={() => setShowSettings(!showSettings)}>
            {showSettings ? "Hide" : "Edit"}
          </button>
        </div>

        {showSettings ? (
          <div className="settings-form">
            <div className="form-row">
              <label>Max HR:</label>
              <input
                type="number"
                value={maxHrInput}
                onChange={(e) => setMaxHrInput(e.target.value)}
                placeholder="e.g., 190"
              />
            </div>
            <div className="form-row">
              <label>LTHR:</label>
              <input
                type="number"
                value={lthrInput}
                onChange={(e) => setLthrInput(e.target.value)}
                placeholder="e.g., 170 (or leave blank for 93% of max)"
              />
            </div>
            <div className="button-row">
              <button onClick={saveSettings}>Save</button>
              <button className="secondary" onClick={() => setShowSettings(false)}>Cancel</button>
            </div>
          </div>
        ) : (
          <div className="settings-summary">
            {settings?.max_hr ? (
              <span>Max HR: {settings.max_hr} | LTHR: {settings.lthr || Math.round(settings.max_hr * 0.93)}</span>
            ) : (
              <span className="warning">Set your max HR to enable metric calculations</span>
            )}
          </div>
        )}
      </div>

      {/* Strava Connection Card */}
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

      {/* Metrics Computation Card */}
      {workouts.length > 0 && hasSettings && needsCompute && (
        <div className="card">
          <h2>Compute Metrics</h2>
          <p className="info">
            {workouts.filter(w => w.rtss === null).length} workouts need metrics computed
          </p>
          <button onClick={computeMetrics} disabled={isComputing}>
            {isComputing ? "Computing..." : "Compute Metrics"}
          </button>
          {computeResult && (
            <p className="sync-result">
              Computed metrics for {computeResult.computed} workouts
            </p>
          )}
        </div>
      )}

      {/* Workouts List */}
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
                  {workout.activity_type === "Run" && workout.pace_min_per_km && (
                    <span className="pace">{formatPace(workout.pace_min_per_km)}</span>
                  )}
                  {workout.activity_type === "Ride" && workout.average_watts && (
                    <span className="power">{workout.average_watts.toFixed(0)}W</span>
                  )}
                  {workout.average_heartrate && (
                    <span>{workout.average_heartrate} bpm</span>
                  )}
                </div>
                {/* Computed metrics row */}
                {workout.rtss !== null && (
                  <div className="workout-metrics">
                    <span className="hr-zone">{workout.hr_zone}</span>
                    <span className="rtss">rTSS: {formatRtss(workout.rtss)}</span>
                    {workout.suffer_score && (
                      <span className="suffer-score">{workout.suffer_score} effort</span>
                    )}
                  </div>
                )}
              </div>
            ))}
          </div>
        </div>
      )}
    </main>
  );
}

export default App;
