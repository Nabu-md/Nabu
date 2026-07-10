// Content script that extracts selected text or full page content
chrome.runtime.onMessage.addListener((request, _sender, sendResponse) => {
  if (request.action === 'clip') {
    const selection = window.getSelection().toString().trim();
    if (selection) {
      sendResponse({ markdown: selection });
      return;
    }
    
    // Extract main article content using Readability-like approach
    const article = document.querySelector('article') || 
                   document.querySelector('[role="main"]') ||
                   document.body;
    
    const markdown = extractToMarkdown(article);
    sendResponse({ markdown });
  }
});

function extractToMarkdown(element) {
  const results = [];
  
  function walk(node) {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent?.trim();
      if (text) results.push(text);
      return;
    }
    
    if (node.nodeType === Node.ELEMENT_NODE) {
      const el = node;
      const tag = el.tagName.toLowerCase();
      
      // Skip script, style, nav, etc.
      if (['script', 'style', 'nav', 'header', 'footer'].includes(tag)) {
        return;
      }
      
      // Process children
      for (const child of el.childNodes) {
        walk(child);
      }
    }
  }
  
  walk(element);
  return results.join('\n\n');
}