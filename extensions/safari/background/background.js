// Background script for Nabu Capture extension
// Handles native messaging communication with the native host

const NATIVE_HOST_NAME = 'com.nabu.capture.host';

// Keep track of pending requests
const pendingRequests = new Map();
let requestIdCounter = 0;

// Connect to native messaging host
let port = null;

function connectToNativeHost() {
  try {
    port = browser.runtime.connectNative(NATIVE_HOST_NAME);
    
    port.onMessage.addListener((response) => {
      if (response.requestId && pendingRequests.has(response.requestId)) {
        const { resolve } = pendingRequests.get(response.requestId);
        pendingRequests.delete(response.requestId);
        resolve(response);
      }
    });

    port.onDisconnect.addListener(() => {
      console.error('Native messaging host disconnected');
      port = null;
      
      // Reject all pending requests
      for (const [requestId, { reject }] of pendingRequests) {
        pendingRequests.delete(requestId);
        reject(new Error('Native messaging host disconnected'));
      }
    });
  } catch (error) {
    console.error('Failed to connect to native host:', error);
  }
}

// Initialize connection
connectToNativeHost();

// Send message to native host and wait for response
function sendMessageToNativeHost(message) {
  return new Promise((resolve, reject) => {
    if (!port) {
      reject(new Error('Not connected to native host'));
      return;
    }

    const requestId = ++requestIdCounter;
    message.requestId = requestId;
    
    pendingRequests.set(requestId, { resolve, reject });
    
    try {
      port.postMessage(message);
    } catch (error) {
      pendingRequests.delete(requestId);
      reject(error);
    }
  });
}

// Listen for messages from popup and content scripts
browser.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.command === 'capture') {
    sendMessageToNativeHost(message)
      .then(response => sendResponse({ success: true, data: response }))
      .catch(error => sendResponse({ success: false, error: error.message }));
    return true; // Indicates async response
  }
  
  return false;
});

// Listen for capture requests from content scripts
browser.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message.type === 'CAPTURE_REQUEST') {
    sendMessageToNativeHost({
      command: 'capture',
      captureType: message.captureType,
      payload: message.payload
    })
      .then(response => sendResponse({ success: true, data: response }))
      .catch(error => sendResponse({ success: false, error: error.message }));
    return true;
  }
  
  return false;
});
