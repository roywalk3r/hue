export class AppError extends Error {
  constructor(message, statusCode = 500, details = null) {
    super(message);
    this.name = 'AppError';
    this.statusCode = statusCode;
    this.details = details;
  }
}

function createAppErrorFactory(name, statusCode, defaultMessage) {
  function NamedAppError(message = defaultMessage, details = null) {
    const error = new AppError(message, statusCode, details);
    error.name = name;
    return error;
  }

  return NamedAppError;
}

export const NotFoundError = createAppErrorFactory('NotFoundError', 404, 'resource not found');
export const validationError = createAppErrorFactory('validationError', 400, 'validaion failed');
