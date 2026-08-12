export type LabProgressStatus = "not_started" | "in_progress" | "completed";

export type LabDefinition = {
  id: string;
  position: number;
  title: string;
  summary: string;
  doc_slug: string;
  template_id?: string | null;
};

export type LabProgress = {
  lab: LabDefinition;
  status: LabProgressStatus;
  attempts: number;
  latest_stage?: string | null;
  latest_feedback: string[];
  last_attempt_at?: string | null;
  completed_at?: string | null;
};

export type StudentLearningOverview = {
  username: string;
  completed_labs: number;
  total_labs: number;
  total_attempts: number;
  last_activity_at?: string | null;
  labs: LabProgress[];
};

export type TeacherLearningOverview = {
  generated_at: string;
  total_labs: number;
  active_students: number;
  students: StudentLearningOverview[];
};

export type LabAttempt = {
  id: string;
  username: string;
  lab_id: string;
  template_id?: string | null;
  source: string;
  source_sha256: string;
  run_success: boolean;
  stage: string;
  attach_expected: boolean;
  attach_verified: boolean;
  completed: boolean;
  feedback: string[];
  created_at: string;
};

export type TeacherStudentAttempts = {
  username: string;
  attempts: LabAttempt[];
};

export const labTitleKey = (labId: string): string =>
  `learn.sectionLab${labId.slice(0, 2)}Title`;
