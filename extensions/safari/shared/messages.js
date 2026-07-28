// Shared message types for Nabu Capture extension

const ALLOWED_COMMANDS = ['capture'];

const MAX_PAYLOAD_SIZE = 1024 * 1024; // 1MB

function validateMessage(message) {
  if (!message || typeof message !== 'object') {
    return { valid: false, error: 'Invalid message format' };
  }

  if (!ALLOWED_COMMANDS.includes(message.command)) {
    return { valid: false, error: `Unknown command: ${message.command}` };
  }

  if (message.payload && typeof message.payload === 'string') {
    if (message.payload.length > MAX_PAYLOAD_SIZE) {
      return { valid: false, error: 'Payload exceeds maximum size' };
    }
  }

  return { valid: true };
}

function createCaptureMessage(captureType, data) {
  return {
    command: 'capture',
    captureType,
    payload: data
  };
}

function createSuccessResponse(requestId, result) {
  return {
    requestId,
    success: true,
    result
  };
}

function createErrorResponse(requestId, error) {
  return {
    requestId,
    success: false,
    error
  };
}

// Export for use in other scripts
if (typeof window !== 'undefined') {
  window.NabuMessages = {
    validateMessage,
    createCaptureMessage,
    createSuccessResponse,
    createErrorResponse,
    ALLOWED_COMMANDS,
    MAX_PAYLOAD_SIZE
  };
}
