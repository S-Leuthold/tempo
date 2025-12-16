import { Line } from 'react-chartjs-2';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
  ChartOptions
} from 'chart.js';
import type {
  RecoverySignals,
  RecoveryBand,
  SignalAxis,
  RecoveryConstraints,
  RecoveryFlag,
  IntensityCap,
  DurationBias,
  OuraHistoryPoint
} from '../types/recovery';

ChartJS.register(
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend
);

interface RecoveryTabProps {
  recoverySignals: RecoverySignals | null;
  ouraHistory: OuraHistoryPoint[];
  isLoading: boolean;
}

export function RecoveryTab({ recoverySignals, ouraHistory, isLoading }: RecoveryTabProps) {
  if (isLoading) {
    return <div className="recovery-loading">Loading recovery data...</div>;
  }

  if (!recoverySignals) {
    return (
      <div className="recovery-empty">
        <p>Connect Oura to see recovery metrics</p>
        <p className="recovery-empty-hint">
          Recovery signals provide detailed insights into your sleep, HRV, and resting heart rate trends.
        </p>
      </div>
    );
  }

  return (
    <div className="recovery-tab">
      {/* Recovery Band Badge */}
      <RecoveryBandBadge
        band={recoverySignals.overall_band}
        lastUpdated={recoverySignals.last_updated}
      />

      {/* Signal Cards Grid */}
      <div className="signals-grid">
        <SignalCard
          title="Sleep"
          icon="💤"
          axis={recoverySignals.sleep}
          unit="h"
          showDebt={true}
          history={ouraHistory}
          chartType="sleep"
        />

        <SignalCard
          title="HRV"
          icon="❤️"
          axis={recoverySignals.hrv}
          unit="ms"
          showDebt={false}
          history={ouraHistory}
          chartType="hrv"
        />

        <SignalCard
          title="Resting HR"
          icon="🫀"
          axis={recoverySignals.rhr}
          unit="bpm"
          showDebt={false}
          history={ouraHistory}
          chartType="rhr"
        />
      </div>

      {/* Recovery Constraints */}
      <ConstraintsCard constraints={recoverySignals.constraints} />

      {/* Recovery Flags */}
      <FlagsCard flags={recoverySignals.flags} />
    </div>
  );
}

// ## ---------------------------------------------------------------------------
// ## Recovery Band Badge Component
// ## ---------------------------------------------------------------------------

interface RecoveryBandBadgeProps {
  band: RecoveryBand;
  lastUpdated: string;
}

function RecoveryBandBadge({ band, lastUpdated }: RecoveryBandBadgeProps) {
  const config = {
    Green: { emoji: '🟢', color: '#22c55e', label: 'GREEN', description: 'Optimal recovery' },
    Yellow: { emoji: '🟡', color: '#eab308', label: 'YELLOW', description: 'Minor recovery concerns' },
    Orange: { emoji: '🟠', color: '#f97316', label: 'ORANGE', description: 'Elevated recovery stress' },
    Red: { emoji: '🔴', color: '#ef4444', label: 'RED', description: 'Significant recovery deficit' }
  };

  const { emoji, color, label, description } = config[band];

  const formatTimestamp = (timestamp: string) => {
    const date = new Date(timestamp);
    const now = new Date();
    const diffMs = now.getTime() - date.getTime();
    const diffHrs = Math.floor(diffMs / (1000 * 60 * 60));

    if (diffHrs < 1) return 'Just now';
    if (diffHrs === 1) return '1 hour ago';
    if (diffHrs < 24) return `${diffHrs} hours ago`;
    const diffDays = Math.floor(diffHrs / 24);
    if (diffDays === 1) return '1 day ago';
    return `${diffDays} days ago`;
  };

  return (
    <div className="recovery-band-badge" style={{ borderColor: color }}>
      <div className="band-header">
        <span className="band-emoji">{emoji}</span>
        <div className="band-info">
          <span className="band-label">Recovery Status: {label}</span>
          <span className="band-description">{description}</span>
        </div>
      </div>
      <span className="band-timestamp">Last updated: {formatTimestamp(lastUpdated)}</span>
    </div>
  );
}

// ## ---------------------------------------------------------------------------
// ## Signal Card Component
// ## ---------------------------------------------------------------------------

interface SignalCardProps {
  title: string;
  icon: string;
  axis: SignalAxis;
  unit: string;
  showDebt: boolean;
  history: OuraHistoryPoint[];
  chartType: 'sleep' | 'hrv' | 'rhr';
}

function SignalCard({ title, icon, axis, unit, showDebt, history, chartType }: SignalCardProps) {
  const getDeltaClass = (delta?: number, inverse = false): string => {
    if (!delta) return 'neutral';
    const threshold = 2;
    if (Math.abs(delta) < threshold) return 'neutral';

    // For HRV, negative delta is bad. For RHR, positive delta is bad.
    // For sleep, use debt instead of state_vs_28d
    if (chartType === 'hrv') {
      return delta < 0 ? 'warning' : 'good';
    } else if (chartType === 'rhr') {
      return delta > 0 ? 'warning' : 'good';
    }
    return 'neutral';
  };

  return (
    <div className="signal-card">
      <h4>{icon} {title}</h4>

      <div className="metric-rows">
        <div className="metric-row">
          <span className="label">Current:</span>
          <span className="value">
            {axis.current_value?.toFixed(chartType === 'sleep' ? 1 : 0) || '-'}{unit}
          </span>
        </div>

        <div className="metric-row">
          <span className="label">7d Avg:</span>
          <span className="value">
            {axis.avg_7d?.toFixed(chartType === 'sleep' ? 1 : 0) || '-'}{unit}
          </span>
        </div>

        <div className="metric-row">
          <span className="label">28d Baseline:</span>
          <span className="value">
            {axis.avg_28d?.toFixed(chartType === 'sleep' ? 1 : 0) || '-'}{unit}
          </span>
        </div>

        {axis.state_vs_28d !== undefined && axis.state_vs_28d !== null && (
          <div className={`metric-row delta-row ${getDeltaClass(axis.state_vs_28d)}`}>
            <span className="label">⚠️ vs Baseline:</span>
            <span className="value delta-value">
              {axis.state_vs_28d > 0 ? '+' : ''}{axis.state_vs_28d.toFixed(1)}{unit}
            </span>
          </div>
        )}

        {showDebt && axis.debt !== undefined && axis.debt > 0 && (
          <div className="debt-indicator">
            💤 Sleep Debt: {axis.debt.toFixed(1)}h over 7 days
          </div>
        )}

        {axis.consecutive_declining_days !== undefined && axis.consecutive_declining_days > 2 && (
          <div className="trend-warning">
            📉 {chartType === 'hrv' ? 'Declining' : 'Rising'} {axis.consecutive_declining_days} days
          </div>
        )}
      </div>

      {history.length > 0 && (
        <div className="chart-container">
          <Line data={createChartData(history, chartType)} options={chartOptions} />
        </div>
      )}
    </div>
  );
}

// ## ---------------------------------------------------------------------------
// ## Constraints Card Component
// ## ---------------------------------------------------------------------------

interface ConstraintsCardProps {
  constraints: RecoveryConstraints;
}

function ConstraintsCard({ constraints }: ConstraintsCardProps) {
  const intensityLabels: Record<IntensityCap, string> = {
    Normal: '✅ No restrictions',
    ModerateOnly: '🟡 Moderate intensity max (Z1-Z3)',
    EasyOnly: '🟠 Easy only (Z1-Z2)',
    RecoveryOnly: '🔴 Recovery only (Z1)'
  };

  const durationLabels: Record<DurationBias, string> = {
    Short: '⬇️ Short sessions preferred',
    Standard: '↔️ Standard durations OK',
    Long: '⬆️ Long sessions supported'
  };

  return (
    <div className="constraints-card">
      <h3>🔍 Recovery Constraints</h3>

      <div className="constraints-grid">
        <div className="constraint-item">
          <span className="constraint-label">Intensity Cap</span>
          <span className={`constraint-value intensity-${constraints.intensity_cap.toLowerCase()}`}>
            {intensityLabels[constraints.intensity_cap]}
          </span>
        </div>

        <div className="constraint-item">
          <span className="constraint-label">Duration Bias</span>
          <span className={`constraint-value duration-${constraints.duration_bias.toLowerCase()}`}>
            {durationLabels[constraints.duration_bias]}
          </span>
        </div>

        <div className="constraint-item">
          <span className="constraint-label">Progression Gate</span>
          <span className={`constraint-value gate-${constraints.progression_gate ? 'active' : 'inactive'}`}>
            {constraints.progression_gate ? '⚠️ ACTIVE - hold progressions' : '✅ INACTIVE'}
          </span>
        </div>
      </div>

      {constraints.caution_note && (
        <div className="caution-note">
          <strong>⚠️ Caution:</strong> {constraints.caution_note}
        </div>
      )}
    </div>
  );
}

// ## ---------------------------------------------------------------------------
// ## Flags Card Component
// ## ---------------------------------------------------------------------------

interface FlagsCardProps {
  flags: RecoveryFlag[];
}

function FlagsCard({ flags }: FlagsCardProps) {
  const flagInfo: Record<RecoveryFlag, { icon: string; label: string; description: string }> = {
    SleepDebtAccumulating: {
      icon: '💤',
      label: 'Sleep Debt Accumulating',
      description: 'Sleep debt has accumulated over recent days. Prioritize sleep duration to restore baseline.'
    },
    HrvBelowBaseline: {
      icon: '⚠️',
      label: 'HRV Below Baseline',
      description: 'Heart rate variability is significantly below your 28-day baseline, indicating increased stress or incomplete recovery.'
    },
    RhrElevated: {
      icon: '🫀',
      label: 'Resting HR Elevated',
      description: 'Resting heart rate is elevated compared to baseline, which may indicate fatigue, overreaching, or illness.'
    },
    MultiDayDecline: {
      icon: '📉',
      label: 'Multi-Day Decline',
      description: 'Recovery metrics have declined for multiple consecutive days, suggesting cumulative fatigue.'
    },
    DataStale: {
      icon: '⏰',
      label: 'Data Stale',
      description: 'Recovery data is outdated (>36 hours old). Sync your Oura ring to get current recovery status.'
    }
  };

  if (flags.length === 0) {
    return (
      <div className="flags-card flags-empty">
        <h3>✅ No Active Recovery Flags</h3>
        <p>All recovery metrics are within normal ranges. Keep up the good recovery practices!</p>
      </div>
    );
  }

  return (
    <div className="flags-card">
      <h3>⚠️ Active Recovery Flags</h3>
      <div className="flags-list">
        {flags.map((flag, idx) => {
          const info = flagInfo[flag];
          return (
            <div key={idx} className="flag-item">
              <span className="flag-icon">{info.icon}</span>
              <div className="flag-content">
                <strong className="flag-label">{info.label}</strong>
                <p className="flag-description">{info.description}</p>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

// ## ---------------------------------------------------------------------------
// ## Chart Creation Functions
// ## ---------------------------------------------------------------------------

function createChartData(history: OuraHistoryPoint[], type: 'sleep' | 'hrv' | 'rhr') {
  const configs = {
    sleep: {
      label: 'Sleep Duration (hrs)',
      data: history.map(h => h.sleep_hours || null),
      borderColor: 'rgb(75, 192, 192)',
      backgroundColor: 'rgba(75, 192, 192, 0.1)'
    },
    hrv: {
      label: 'HRV (ms)',
      data: history.map(h => h.hrv || null),
      borderColor: 'rgb(255, 99, 132)',
      backgroundColor: 'rgba(255, 99, 132, 0.1)'
    },
    rhr: {
      label: 'Resting HR (bpm)',
      data: history.map(h => h.resting_hr || null),
      borderColor: 'rgb(54, 162, 235)',
      backgroundColor: 'rgba(54, 162, 235, 0.1)'
    }
  };

  const config = configs[type];

  return {
    labels: history.map(h => formatDate(h.date)),
    datasets: [{
      label: config.label,
      data: config.data,
      borderColor: config.borderColor,
      backgroundColor: config.backgroundColor,
      tension: 0.1,
      spanGaps: true
    }]
  };
}

function formatDate(dateStr: string): string {
  const date = new Date(dateStr);
  return date.toLocaleDateString(undefined, { month: 'short', day: 'numeric' });
}

// ## ---------------------------------------------------------------------------
// ## Chart Configuration
// ## ---------------------------------------------------------------------------

const chartOptions: ChartOptions<'line'> = {
  responsive: true,
  maintainAspectRatio: false,
  plugins: {
    legend: {
      display: false
    },
    tooltip: {
      mode: 'index',
      intersect: false
    }
  },
  scales: {
    x: {
      grid: {
        display: false
      }
    },
    y: {
      beginAtZero: false,
      grid: {
        color: 'rgba(0, 0, 0, 0.05)'
      }
    }
  }
};
