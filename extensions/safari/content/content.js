// Content script for Nabu Capture extension
// Injects capture functionality into web pages

(function() {
  'use strict';

  // Listen for capture requests from the extension
  browser.runtime.onMessage.addListener((message, sender, sendResponse) => {
    if (message.type === 'CAPTURE_PAGE') {
      try {
        const captureData = {
          url: window.location.href,
          title: document.title,
          html: document.documentElement.outerHTML,
          selectedText: window.getSelection().toString()
        };
        sendResponse({ success: true, data: captureData });
      } catch (error) {
        sendResponse({ success: false, error: error.message });
      }
      return true;
    }
    
    if (message.type === 'CAPTURE_SELECTION') {
      try {
        const selection = window.getSelection();
        const selectedText = selection.toString();
        
        if (!selectedText) {
          sendResponse({ success: false, error: 'No text selected' });
          return true;
        }
        
        const captureData = {
          url: window.location.href,
          title: document.title,
          selectedText: selectedText
        };
        sendResponse({ success: true, data: captureData });
      } catch (error) {
        sendResponse({ success: false, error: error.message });
      }
      return true;
    }
    
    return false;
  });

  // Expose capture functions globally for popup access
  window.nabuCapture = {
    capturePage: function() {
      return new Promise((resolve, reject) => {
        browser.runtime.sendMessage({ type: 'CAPTURE_PAGE' })
          .then(response => {
            if (response.success) {
              resolve(response.data);
            } else {
              reject(new Error(response.error));
            }
          })
          .catch(error => reject(error));
      });
    },
    
    captureSelection: function() {
      return new Promise((resolve, reject) => {
        browser.runtime.sendMessage({ type: 'CAPTURE_SELECTION' })
          .then(response => {
            if (response.success) {
              resolve(response.data);
            } else {
              reject(new Error(response.error));
            }
          })
          .catch(error => reject(error));
      });
    }
  };
})();
