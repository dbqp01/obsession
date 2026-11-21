export class WorkflowError extends Error {
  constructor(code, status = 502, details = {}) {
    super(code);
    this.code = code;
    this.status = status;
    Object.assign(this, details);
  }
}
