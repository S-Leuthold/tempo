import "./CoachCards.css";
import type { WorkoutAnalysisV4 } from "../types/analysis";

interface CoachCardsProps {
  analysis: WorkoutAnalysisV4;
}

export function CoachCards({ analysis }: CoachCardsProps) {
  return (
    <div className="coach-cards">
      {/* Card 1: Performance */}
      <div className="coach-card performance-card">
        <div className="card-header">
          <span className="card-icon">📊</span>
          <h3>Performance</h3>
        </div>
        <div className="card-content">
          <div className="metric-comparison">
            <span className="comparison-label">
              {analysis.performance.comparison_date}: {analysis.performance.comparison_value}
            </span>
            <span className="delta">{analysis.performance.delta}</span>
            <span className="today-value">Today: {analysis.performance.today_value}</span>
          </div>
          <p>{analysis.performance.insight}</p>
        </div>
      </div>

      {/* Card 2: HR & Efficiency */}
      <div className="coach-card hr-card">
        <div className="card-header">
          <span className="card-icon">❤️</span>
          <h3>HR & Efficiency</h3>
        </div>
        <div className="card-content">
          <div className="hr-stats">
            <span className="hr-value">{analysis.hr_efficiency.avg_hr} BPM</span>
            <span className={`zone-badge zone-${analysis.hr_efficiency.hr_zone.toLowerCase()}`}>
              {analysis.hr_efficiency.hr_zone}
            </span>
            <span className="hr-pct">{analysis.hr_efficiency.hr_pct_max}% max</span>
          </div>
          <p>{analysis.hr_efficiency.hr_assessment}</p>
          {analysis.hr_efficiency.efficiency_trend && (
            <p className="efficiency-note">{analysis.hr_efficiency.efficiency_trend}</p>
          )}
        </div>
      </div>

      {/* Card 3: Training Status */}
      <div className="coach-card status-card">
        <div className="card-header">
          <span className="card-icon">🏃</span>
          <h3>Training Status</h3>
        </div>
        <div className="card-content">
          <div className={`tsb-row tsb-${getTsbClass(analysis.training_status.tsb_band)}`}>
            <span className="tsb-emoji">{getTsbEmoji(analysis.training_status.tsb_value)}</span>
            <span>
              TSB: {analysis.training_status.tsb_value.toFixed(0)} ({analysis.training_status.tsb_band.replace('_', ' ')})
            </span>
          </div>
          <p className="tsb-assessment">{analysis.training_status.tsb_assessment}</p>
          {analysis.training_status.top_flags.length > 0 && (
            <div className="flags-section">
              {analysis.training_status.top_flags.map((flag, i) => (
                <div key={i} className="flag-row">
                  <span className="flag-emoji">⚠️</span>
                  <span>{flag.replace('_', ' ')}</span>
                </div>
              ))}
            </div>
          )}
          <div className="status-metrics">
            <div className="status-row">
              <span className="status-emoji">📊</span>
              <span>{analysis.training_status.adherence_note}</span>
            </div>
            <div className="status-row">
              <span className="status-emoji">🔄</span>
              <span>{analysis.training_status.progression_state}</span>
            </div>
          </div>
        </div>
      </div>

      {/* Card 4: Tomorrow */}
      <div className="coach-card tomorrow-card">
        <div className="card-header">
          <span className="card-icon">📅</span>
          <h3>Tomorrow</h3>
        </div>
        <div className="card-content">
          <div className="tomorrow-prescription">
            <div className="activity-line">
              <strong className="activity-type">{analysis.tomorrow.activity_type}</strong>
              <span className="duration">{analysis.tomorrow.duration_min} min</span>
              <span className="duration-badge">{analysis.tomorrow.duration_label}</span>
            </div>
            <div className="prescription-details">
              <div className="detail-row">
                <span className="detail-label">Intensity:</span>
                <span>{analysis.tomorrow.intensity}</span>
              </div>
              <div className="detail-row">
                <span className="detail-label">Goal:</span>
                <span>{analysis.tomorrow.goal.replace('_', ' ')}</span>
              </div>
            </div>
          </div>
          <p className="tomorrow-rationale">{analysis.tomorrow.rationale}</p>
          <div className={`confidence-badge confidence-${analysis.tomorrow.confidence}`}>
            Confidence: {analysis.tomorrow.confidence}
          </div>
        </div>
      </div>

      {/* Card 5: Eyes On (conditional) */}
      {analysis.eyes_on && analysis.eyes_on.priorities.length > 0 && (
        <div className="coach-card eyes-on-card">
          <div className="card-header">
            <span className="card-icon">👀</span>
            <h3>Eyes On</h3>
          </div>
          <div className="card-content">
            {analysis.eyes_on.priorities.map((item, i) => (
              <div key={i} className="priority-item">
                <div className="priority-header">
                  <strong className="flag-name">{item.flag.replace('_', ' ')}</strong>
                  {item.current_value && (
                    <span className="current-value">{item.current_value}</span>
                  )}
                </div>
                <div className="priority-body">
                  <div className="priority-action">{item.action}</div>
                  <div className="priority-threshold">Target: {item.threshold}</div>
                  <div className="priority-why">{item.why_it_matters}</div>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

// Helper functions
function getTsbClass(band: string): string {
  switch (band) {
    case "fresh":
      return "fresh";
    case "slightly_fatigued":
      return "slight";
    case "moderate_fatigue":
      return "moderate";
    case "high_fatigue":
      return "high";
    default:
      return "neutral";
  }
}

function getTsbEmoji(tsb: number): string {
  if (tsb > 0) return "🟢";
  if (tsb > -10) return "🟡";
  if (tsb > -20) return "🟠";
  return "🔴";
}
