const http = require('http');
const https = require('https');

const PORT = process.env.PORT || 8080;
const ARTIFACT_REGISTRY = 'us-central1-npm.pkg.dev';
const PROJECT = 'distributable-octet-pipeline';
const REPOSITORY = 'npm-packages';

// Cache for access token
let cachedToken = null;
let tokenExpiry = 0;

// Fetch access token from metadata server
async function getAccessToken() {
  const now = Date.now();

  // Return cached token if still valid (with 5 min buffer)
  if (cachedToken && tokenExpiry > now + 300000) {
    return cachedToken;
  }

  return new Promise((resolve, reject) => {
    const options = {
      hostname: 'metadata.google.internal',
      path: '/computeMetadata/v1/instance/service-accounts/default/token',
      headers: {
        'Metadata-Flavor': 'Google'
      }
    };

    http.get(options, (res) => {
      let data = '';
      res.on('data', (chunk) => data += chunk);
      res.on('end', () => {
        try {
          const json = JSON.parse(data);
          cachedToken = json.access_token;
          tokenExpiry = now + (json.expires_in * 1000);
          console.log('Fetched new access token, expires in', json.expires_in, 'seconds');
          resolve(cachedToken);
        } catch (err) {
          reject(err);
        }
      });
    }).on('error', reject);
  });
}

// Proxy server
const server = http.createServer(async (req, res) => {
  // Health check
  if (req.url === '/health') {
    res.writeHead(200, { 'Content-Type': 'text/plain' });
    res.end('healthy\n');
    return;
  }

  try {
    // Get access token
    const token = await getAccessToken();

    // Proxy request to Artifact Registry
    const options = {
      hostname: ARTIFACT_REGISTRY,
      path: `/${PROJECT}/${REPOSITORY}${req.url}`,
      method: req.method,
      headers: {
        ...req.headers,
        'Host': ARTIFACT_REGISTRY,
        'Authorization': `Bearer ${token}`
      }
    };

    // Remove host header from incoming request
    delete options.headers['host'];

    const proxyReq = https.request(options, (proxyRes) => {
      // Forward status and headers
      res.writeHead(proxyRes.statusCode, proxyRes.headers);

      // Pipe response
      proxyRes.pipe(res);
    });

    proxyReq.on('error', (err) => {
      console.error('Proxy error:', err);
      res.writeHead(502, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({ error: 'Bad Gateway', details: err.message }));
    });

    // Pipe request body
    req.pipe(proxyReq);

  } catch (err) {
    console.error('Token fetch error:', err);
    res.writeHead(500, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'Internal Server Error', details: err.message }));
  }
});

server.listen(PORT, () => {
  console.log(`Artifact Registry proxy listening on port ${PORT}`);
  console.log(`Proxying to: https://${ARTIFACT_REGISTRY}/${PROJECT}/${REPOSITORY}/`);
});
