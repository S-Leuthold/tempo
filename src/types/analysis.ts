// V4 Multi-Card Analysis Types

export interface WorkoutAnalysisV4 {
  performance: PerformanceCard;
  hr_efficiency: HrEfficiencyCard;
  training_status: TrainingStatusCard;
  tomorrow: TomorrowCard;
  eyes_on?: EyesOnCard;
}

export interface PerformanceCard {
  metric_name: string;
  comparison_date: string;
  comparison_value: string;
  today_value: string;
  delta: string;
  insight: string;
}

export interface HrEfficiencyCard {
  avg_hr: number;
  hr_zone: string;
  hr_pct_max: number;
  hr_assessment: string;
  efficiency_trend?: string;
}

export interface TrainingStatusCard {
  tsb_value: number;
  tsb_band: string;
  tsb_assessment: string;
  top_flags: string[];
  adherence_note: string;
  progression_state: string;
}

export interface TomorrowCard {
  activity_type: string;
  duration_min: number;
  duration_label: string;
  intensity: string;
  goal: string;
  rationale: string;
  confidence: string;
}

export interface EyesOnCard {
  priorities: FlagPriority[];
}

export interface FlagPriority {
  flag: string;
  current_value?: string;
  threshold: string;
  action: string;
  why_it_matters: string;
}
