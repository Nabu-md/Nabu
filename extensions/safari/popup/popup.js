// Popup script for Nabu Capture extension

document.addEventListener('DOMContentLoaded', function() {
  const capturePageBtn = document.getElementById('capture-page');
  const captureSelectionBtn = document.getElementById('capture-selection');
  const captureFullBtn = document.getElementById('capture-full');
  const captureScreenshotBtn = document.getElementById('capture-screenshot');
  const captureReaderBtn = document.getElementById('capture-reader');
  const statusEl = document.getElementById('status');
  const resultEl = document.getElementById('result');
  const resultContent = document.getElementById('result-content');
  const errorEl = document.getElementById('error');
  const errorContent = document.getElementById('error-content');

  function setStatus(text, type = '') {
    statusEl.textContent = text;
    statusEl.className = 'status' + (type ? ' ' + type : '');
  }

  function showResult(message) {
    resultContent.textContent = message;
    resultEl.classList.remove('hidden');
    errorEl.classList.add('hidden');
  }

  function showError(message) {
    errorContent.textContent = message;
    errorEl.classList.remove('hidden');
    resultEl.classList.add('hidden');
  }

  function hideMessages() {
    resultEl.classList.add('hidden');
    errorEl.classList.add('hidden');
  }

  function setButtonsEnabled(enabled) {
    capturePageBtn.disabled = !enabled;
    captureSelectionBtn.disabled = !enabled;
    captureFullBtn.disabled = !enabled;
    captureScreenshotBtn.disabled = !enabled;
    captureReaderBtn.disabled = !enabled;
  }

  async function capture(captureType, payload = {}) {
    hideMessages();
    setStatus('Capturing...', 'loading');
    setButtonsEnabled(false);

    try {
      // Get current tab
      const [tab] = await browser.tabs.query({ active: true, currentWindow: true });
      
      if (!tab) {
        throw new Error('No active tab found');
      }

      // Determine which content script message to send
      let messageType = 'CAPTURE_PAGE';
      if (captureType === 'note') {
        messageType = 'CAPTURE_SELECTION';
      } else if (captureType === 'reader_mode') {
        messageType = 'CAPTURE_READER';
      }

      // Send message to content script
      const response = await browser.tabs.sendMessage(tab.id, {
        type: messageType
      });

      if (!response.success) {
        throw new Error(response.error || 'Capture failed');
      }

      const captureData = response.data;

      // For screen capture, don't send content script data
      let nativePayload = { ...captureData, ...payload };
      if (captureType === 'screen_capture') {
        nativePayload = { ...payload, url: tab.url, title: payload.title || 'Screen Capture' };
      }

      // Send to native host via background script
      const nativeResponse = await browser.runtime.sendMessage({
        command: 'capture',
        captureType: captureType,
        payload: nativePayload
      });

      if (nativeResponse.success) {
        const result = nativeResponse.data;
        if (result.success) {
          setStatus('Captured!', '');
          showResult(`Successfully captured: ${captureData.title || captureData.url}`);
        } else {
          setStatus('Failed', 'error');
          showError(result.error || 'Capture failed');
        }
      } else {
        setStatus('Error', 'error');
        showError(nativeResponse.error || 'Communication error');
      }
    } catch (error) {
      setStatus('Error', 'error');
      showError(error.message || 'An error occurred');
    } finally {
      setButtonsEnabled(true);
    }
  }

  capturePageBtn.addEventListener('click', () => {
    capture('bookmark');
  });

  captureSelectionBtn.addEventListener('click', () => {
    capture('note');
  });

  captureFullBtn.addEventListener('click', () => {
    capture('document');
  });

  captureScreenshotBtn.addEventListener('click', () => {
    capture('screen_capture', { text: '', title: 'Screen Capture' });
  });

  captureReaderBtn.addEventListener('click', () => {
    capture('reader_mode');
  });
});
