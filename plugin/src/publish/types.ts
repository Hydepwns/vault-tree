export interface PostMetadata {
  title: string;
  date: string;
  description: string;
  tags: string[];
  slug: string;
  author?: string;
  pattern_style?: string;
  series?: string;
  series_order?: number;
  reading_time?: string;
}

export interface ValidationResult {
  valid: boolean;
  errors: ValidationError[];
  warnings: ValidationWarning[];
}

export interface ValidationError {
  field: string;
  message: string;
}

export interface ValidationWarning {
  field: string;
  message: string;
}

export interface PublishResult {
  success: boolean;
  url?: string;
  error?: string;
}

export interface PostStats {
  wordCount: number;
  characterCount: number;
  estimatedReadingTime: string;
  linkCount: number;
  imageCount: number;
}
