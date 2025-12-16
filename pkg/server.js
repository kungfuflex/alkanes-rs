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

// Fetch package metadata
async function getPackageMetadata(packageName, token) {
  return new Promise((resolve, reject) => {
    const options = {
      hostname: ARTIFACT_REGISTRY,
      path: `/${PROJECT}/${REPOSITORY}/${packageName}`,
      headers: {
        'Host': ARTIFACT_REGISTRY,
        'Authorization': `Bearer ${token}`
      }
    };

    https.get(options, (res) => {
      let data = '';
      res.on('data', (chunk) => data += chunk);
      res.on('end', () => {
        if (res.statusCode === 200) {
          try {
            resolve(JSON.parse(data));
          } catch (err) {
            reject(new Error('Failed to parse package metadata'));
          }
        } else {
          reject(new Error(`Package not found: ${res.statusCode}`));
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

    // Handle /dist/{package} endpoint - direct tarball download
    if (req.url.startsWith('/dist/')) {
      const packageName = req.url.substring(6); // Remove '/dist/'
      console.log(`Fetching tarball for ${packageName}`);

      // Get package metadata
      const metadata = await getPackageMetadata(packageName, token);
      const latestVersion = metadata['dist-tags'].latest;
      const tarballUrl = metadata.versions[latestVersion].dist.tarball;

      // Extract path from tarball URL
      const tarballPath = new URL(tarballUrl).pathname;

      // Proxy tarball request (follow redirects)
      const options = {
        hostname: ARTIFACT_REGISTRY,
        path: tarballPath,
        headers: {
          'Host': ARTIFACT_REGISTRY,
          'Authorization': `Bearer ${token}`
        }
      };

      const proxyReq = https.get(options, (proxyRes) => {
        // Follow redirects (307, 302, 301)
        if (proxyRes.statusCode >= 300 && proxyRes.statusCode < 400 && proxyRes.headers.location) {
          const redirectUrl = proxyRes.headers.location;
          console.log(`Following redirect to: ${redirectUrl}`);

          // Build redirect options (handle both relative and absolute URLs)
          const redirectOptions = redirectUrl.startsWith('http')
            ? redirectUrl  // Absolute URL
            : {            // Relative path
                hostname: ARTIFACT_REGISTRY,
                path: redirectUrl,
                headers: {
                  'Host': ARTIFACT_REGISTRY,
                  'Authorization': `Bearer ${token}`
                }
              };

          https.get(redirectOptions, (finalRes) => {
            res.writeHead(finalRes.statusCode, {
              'Content-Type': finalRes.headers['content-type'] || 'application/octet-stream',
              'Content-Length': finalRes.headers['content-length']
            });
            finalRes.pipe(res);
          }).on('error', (err) => {
            console.error('Redirect fetch error:', err);
            res.writeHead(502, { 'Content-Type': 'application/json' });
            res.end(JSON.stringify({ error: 'Failed to fetch tarball from redirect', details: err.message }));
          });
        } else {
          // Direct response
          res.writeHead(proxyRes.statusCode, proxyRes.headers);
          proxyRes.pipe(res);
        }
      });

      proxyReq.on('error', (err) => {
        console.error('Tarball fetch error:', err);
        res.writeHead(502, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'Failed to fetch tarball', details: err.message }));
      });

      return;
    }

    // Default: proxy request to Artifact Registry
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
