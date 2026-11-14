export interface KeyValidation {
  status: "accepted" | "rejected" | "inconclusive" | "not_configured";
  message: string;
  textModel: string;
  textModelListed: boolean | null;
  realtimeModelListed: boolean | null;
  diagnostic: {
    category: string; httpStatus: number | null; code: string | null; errorType: string | null;
    requestId: string | null; retryAfterSeconds: number | null; description: string; action: string;
  } | null;
}
export interface CredentialStore {
  save(): Promise<boolean>;
  check(): Promise<KeyValidation>;
  delete(): Promise<void>;
}
