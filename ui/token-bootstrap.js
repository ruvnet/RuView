// Dashboard token bootstrap — fetches API token from server-generated config.
// The token is injected at container startup from RUVIEW_API_TOKEN env var.
// Falls back to prompt if config unavailable.
(function(){
  if (localStorage.getItem('ruview-api-token')) return;

  // Try to load from server-generated config first
  var script = document.createElement('script');
  script.src = '/ui/config/dashboard-config.js';
  script.onload = function() {
    var cfg = window.__RUVIEW_DASHBOARD_CONFIG;
    if (cfg && cfg.token) {
      localStorage.setItem('ruview-api-token', cfg.token);
    }
  };
  script.onerror = function() {
    // Config not available — prompt user
    var token = prompt('Enter API token (from RUVIEW_API_TOKEN):');
    if (token) localStorage.setItem('ruview-api-token', token);
  };
  document.head.appendChild(script);
})();
