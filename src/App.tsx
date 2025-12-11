import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";
import { CoachCards } from "./components/CoachCards";
import type { WorkoutAnalysisV4 } from "./types/analysis";

interface StravaAuthStatus {
  is_authenticated: boolean;
  expires_at: string | null;
  needs_refresh: boolean;
}

interface OuraAuthStatus {
  is_authenticated: boolean;
  expires_at: string | null;
  needs_refresh: boolean;
}

interface SyncResult {
  new_activities: number;
  total_fetched: number;
}

interface OuraSyncResult {
  sleep_records: number;
  hrv_records: number;
  resting_hr_records: number;
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

interface WeeklyVolume {
  total_hrs: number;
  run_hrs: number;
  ride_hrs: number;
  other_hrs: number;
}

interface IntensityDistribution {
  z1_pct: number;
  z2_pct: number;
  z3_pct: number;
  z4_pct: number;
  z5_pct: number;
}

interface LongestSession {
  run_min: number | null;
  ride_min: number | null;
}

interface TrainingContext {
  atl: number | null;
  ctl: number | null;
  tsb: number | null;
  weekly_volume: WeeklyVolume;
  week_over_week_delta_pct: number | null;
  intensity_distribution: IntensityDistribution;
  longest_session: LongestSession;
  consistency_pct: number | null;
  workouts_this_week: number;
}

// Legacy format (still stored in DB) - not currently used in frontend
// interface WorkoutAnalysis {
//   id: number | null;
//   workout_id: number;
//   summary: string;
//   tomorrow_recommendation: string;
//   risk_flags: string[];
//   goal_notes: string | null;
//   created_at: string | null;
// }

interface AnalysisResult {
  workout_id: number;
  analysis: WorkoutAnalysisV4;  // V4 multi-card format
  input_tokens: number;
  output_tokens: number;
}

function App() {
  const [stravaStatus, setStravaStatus] = useState<StravaAuthStatus | null>(null);
  const [ouraStatus, setOuraStatus] = useState<OuraAuthStatus | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [isConnectingOura, setIsConnectingOura] = useState(false);
  const [isSyncing, setIsSyncing] = useState(false);
  const [isSyncingOura, setIsSyncingOura] = useState(false);
  const [ouraSyncResult, setOuraSyncResult] = useState<OuraSyncResult | null>(null);
  const [isComputing, setIsComputing] = useState(false);
  const [syncResult, setSyncResult] = useState<SyncResult | null>(null);
  const [computeResult, setComputeResult] = useState<ComputeResult | null>(null);
  const [workouts, setWorkouts] = useState<WorkoutWithMetrics[]>([]);
  const [settings, setSettings] = useState<UserSettings | null>(null);
  const [trainingContext, setTrainingContext] = useState<TrainingContext | null>(null);
  const [latestAnalysis, setLatestAnalysis] = useState<WorkoutAnalysisV4 | null>(null);
  const [isAnalyzing, setIsAnalyzing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [showSettings, setShowSettings] = useState(false);

  // Form state for settings
  const [maxHrInput, setMaxHrInput] = useState("");
  const [lthrInput, setLthrInput] = useState("");

  useEffect(() => {
    checkStravaStatus();
    checkOuraStatus();
    loadWorkouts();
    loadSettings();
    loadTrainingContext();
    loadLatestAnalysis();
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

  async function loadTrainingContext() {
    try {
      const data = await invoke<TrainingContext>("get_training_context");
      setTrainingContext(data);
    } catch (e) {
      console.error("Failed to load training context:", e);
    }
  }

  async function loadLatestAnalysis() {
    try {
      // Note: get_latest_analysis returns legacy format from DB
      // We'll need to re-analyze to get V4 format
      // For now, skip loading old analysis
      setLatestAnalysis(null);
    } catch (e) {
      console.error("Failed to load latest analysis:", e);
    }
  }

  async function analyzeLatestWorkout() {
    if (workouts.length === 0) return;

    setIsAnalyzing(true);
    setError(null);

    try {
      const result = await invoke<AnalysisResult>("analyze_workout", {
        workoutId: workouts[0].id,
      });
      setLatestAnalysis(result.analysis);
    } catch (e: unknown) {
      const errorObj = e as { message?: string };
      setError(`Analysis failed: ${errorObj.message || e}`);
    } finally {
      setIsAnalyzing(false);
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

  async function checkOuraStatus() {
    try {
      const status = await invoke<OuraAuthStatus>("oura_get_auth_status");
      setOuraStatus(status);
    } catch (e) {
      console.error("Failed to check Oura status:", e);
    }
  }

  async function connectOura() {
    setIsConnectingOura(true);
    setError(null);
    try {
      const authUrl = await invoke<string>("oura_start_auth");
      await openUrl(authUrl);
      await invoke("oura_complete_auth");
      await checkOuraStatus();
    } catch (e) {
      setError(`Oura connection failed: ${e}`);
    } finally {
      setIsConnectingOura(false);
    }
  }

  async function disconnectOura() {
    try {
      await invoke("oura_disconnect");
      await checkOuraStatus();
    } catch (e) {
      setError(`Failed to disconnect Oura: ${e}`);
    }
  }

  async function syncOuraData() {
    setIsSyncingOura(true);
    setError(null);
    setOuraSyncResult(null);

    try {
      const result = await invoke<OuraSyncResult>("oura_sync_data");
      setOuraSyncResult(result);
    } catch (e) {
      setError(`Oura sync failed: ${e}`);
    } finally {
      setIsSyncingOura(false);
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
      await loadTrainingContext();
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
      await loadTrainingContext();
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

      {/* Oura Connection Card */}
      <div className="card">
        <h2>Oura Connection</h2>

        {ouraStatus === null ? (
          <p>Loading...</p>
        ) : ouraStatus.is_authenticated ? (
          <div>
            <p className="status connected">Connected</p>
            <div className="button-row">
              <button type="button" onClick={syncOuraData} disabled={isSyncingOura}>
                {isSyncingOura ? "Syncing..." : "Sync Oura Data"}
              </button>
              <button type="button" onClick={disconnectOura} className="secondary">
                Disconnect
              </button>
            </div>
            {ouraSyncResult && (
              <p className="sync-result">
                Synced {ouraSyncResult.sleep_records} sleep, {ouraSyncResult.hrv_records} HRV, {ouraSyncResult.resting_hr_records} resting HR records
              </p>
            )}
          </div>
        ) : (
          <div>
            <p className="status disconnected">Not connected</p>
            <button onClick={connectOura} disabled={isConnectingOura}>
              {isConnectingOura ? "Connecting..." : "Connect Oura"}
            </button>
          </div>
        )}
      </div>

      {/* Training Load Card */}
      {trainingContext && (
        <div className="card">
          <h2>Training Load</h2>
          <div className="training-load-grid">
            <div className="load-metric">
              <span className="load-label">ATL</span>
              <span className="load-value atl">{trainingContext.atl?.toFixed(0) || "-"}</span>
              <span className="load-sublabel">7-day load</span>
            </div>
            <div className="load-metric">
              <span className="load-label">CTL</span>
              <span className="load-value ctl">{trainingContext.ctl?.toFixed(0) || "-"}</span>
              <span className="load-sublabel">Fitness</span>
            </div>
            <div className="load-metric">
              <span className="load-label">TSB</span>
              <span className={`load-value tsb ${trainingContext.tsb !== null ? (trainingContext.tsb < -20 ? "fatigued" : trainingContext.tsb > 5 ? "fresh" : "neutral") : ""}`}>
                {trainingContext.tsb?.toFixed(0) || "-"}
              </span>
              <span className="load-sublabel">Form</span>
            </div>
          </div>

          <div className="weekly-volume">
            <h3>This Week</h3>
            <div className="volume-bar">
              <div className="volume-segment run" style={{ flex: trainingContext.weekly_volume.run_hrs }} />
              <div className="volume-segment ride" style={{ flex: trainingContext.weekly_volume.ride_hrs }} />
              <div className="volume-segment other" style={{ flex: trainingContext.weekly_volume.other_hrs }} />
            </div>
            <div className="volume-legend">
              <span>{trainingContext.weekly_volume.total_hrs.toFixed(1)}h total</span>
              {trainingContext.weekly_volume.run_hrs > 0 && (
                <span className="run-label">{trainingContext.weekly_volume.run_hrs.toFixed(1)}h run</span>
              )}
              {trainingContext.weekly_volume.ride_hrs > 0 && (
                <span className="ride-label">{trainingContext.weekly_volume.ride_hrs.toFixed(1)}h ride</span>
              )}
              {trainingContext.week_over_week_delta_pct !== null && (
                <span className={trainingContext.week_over_week_delta_pct >= 0 ? "delta-up" : "delta-down"}>
                  {trainingContext.week_over_week_delta_pct >= 0 ? "+" : ""}{trainingContext.week_over_week_delta_pct.toFixed(0)}% vs last week
                </span>
              )}
            </div>
          </div>

          <div className="context-stats">
            <span>{trainingContext.workouts_this_week} workouts this week</span>
            {trainingContext.consistency_pct !== null && (
              <span>Consistency: {trainingContext.consistency_pct.toFixed(0)}%</span>
            )}
          </div>
        </div>
      )}

      {/* Coach Analysis Card */}
      {workouts.length > 0 && hasSettings && (
        <div className="card">
          <div className="card-header">
            <h2>Coach Analysis</h2>
            <button
              className="small"
              onClick={analyzeLatestWorkout}
              disabled={isAnalyzing}
            >
              {isAnalyzing ? "Analyzing..." : latestAnalysis ? "Re-analyze" : "Analyze"}
            </button>
          </div>

          {latestAnalysis ? (
            <CoachCards analysis={latestAnalysis} />
          ) : (
            <div className="analysis-empty">
              <p className="info">
                Click "Analyze" to get AI-powered coaching feedback on your latest workout.
              </p>
            </div>
          )}
        </div>
      )}

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
