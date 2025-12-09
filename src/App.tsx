import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { openUrl } from "@tauri-apps/plugin-opener";
import "./App.css";

interface StravaAuthStatus {
  is_authenticated: boolean;
  expires_at: string | null;
  needs_refresh: boolean;
}

function App() {
  const [stravaStatus, setStravaStatus] = useState<StravaAuthStatus | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    checkStravaStatus();
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

  async function connectStrava() {
    setIsConnecting(true);
    setError(null);

    try {
      // Get the authorization URL
      const authUrl = await invoke<string>("strava_start_auth");

      // Open browser for user to authorize
      await openUrl(authUrl);

      // Wait for callback and complete auth
      await invoke("strava_complete_auth");

      // Refresh status
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
            {stravaStatus.expires_at && (
              <p className="expires">
                Token expires: {new Date(stravaStatus.expires_at).toLocaleString()}
              </p>
            )}
            {stravaStatus.needs_refresh && (
              <p className="warning">Token needs refresh</p>
            )}
            <button onClick={disconnectStrava}>Disconnect</button>
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
    </main>
  );
}

export default App;
