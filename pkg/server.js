const http = require('http');
const https = require('https');
const url = require('url');

const PORT = process.env.PORT || 8080;
const ARTIFACT_REGISTRY = 'us-central1-npm.pkg.dev';
const PROJECT = 'distributable-octet-pipeline';
const REPOSITORY = 'npm-packages';

// Cache for access token
let cachedToken = null;
let tokenExpiry = 0;

// Cache for package metadata (5 minute TTL)
const metadataCache = new Map();
const METADATA_CACHE_TTL = 5 * 60 * 1000; // 5 minutes

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

// Fetch package metadata from Artifact Registry
async function getPackageMetadata(packageName, token, forceRefresh = false) {
  const cacheKey = packageName;
  const cached = metadataCache.get(cacheKey);

  if (!forceRefresh && cached && cached.expiry > Date.now()) {
    return cached.data;
  }

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
      // Follow redirects (307, 302, 301)
      if (res.statusCode >= 300 && res.statusCode < 400 && res.headers.location) {
        const redirectUrl = res.headers.location;
        console.log(`Following metadata redirect to: ${redirectUrl}`);

        const redirectOptions = redirectUrl.startsWith('http')
          ? redirectUrl
          : {
              hostname: ARTIFACT_REGISTRY,
              path: redirectUrl,
              headers: {
                'Host': ARTIFACT_REGISTRY,
                'Authorization': `Bearer ${token}`
              }
            };

        https.get(redirectOptions, (finalRes) => {
          let data = '';
          finalRes.on('data', (chunk) => data += chunk);
          finalRes.on('end', () => {
            if (finalRes.statusCode === 200) {
              try {
                const metadata = JSON.parse(data);
                // Cache the metadata
                metadataCache.set(cacheKey, {
                  data: metadata,
                  expiry: Date.now() + METADATA_CACHE_TTL
                });
                resolve(metadata);
              } catch (err) {
                reject(new Error('Failed to parse package metadata'));
              }
            } else {
              reject(new Error(`Package not found after redirect: ${finalRes.statusCode}`));
            }
          });
        }).on('error', reject);
      } else {
        // Direct response
        let data = '';
        res.on('data', (chunk) => data += chunk);
        res.on('end', () => {
          if (res.statusCode === 200) {
            try {
              const metadata = JSON.parse(data);
              // Cache the metadata
              metadataCache.set(cacheKey, {
                data: metadata,
                expiry: Date.now() + METADATA_CACHE_TTL
              });
              resolve(metadata);
            } catch (err) {
              reject(new Error('Failed to parse package metadata'));
            }
          } else {
            reject(new Error(`Package not found: ${res.statusCode}`));
          }
        });
      }
    }).on('error', reject);
  });
}

// Fetch tarball with optional version
async function fetchTarball(res, packageName, version, token) {
  // Get package metadata
  const metadata = await getPackageMetadata(packageName, token);

  // Determine which version to fetch
  let targetVersion;
  if (!version || version === 'latest') {
    // Use latest-dev tag or fall back to latest
    targetVersion = metadata['dist-tags']['latest-dev'] || metadata['dist-tags']['latest'];
  } else {
    // Check if specific version exists
    if (!metadata.versions[version]) {
      res.writeHead(404, { 'Content-Type': 'application/json' });
      res.end(JSON.stringify({
        error: 'Version not found',
        requestedVersion: version,
        availableVersions: Object.keys(metadata.versions).slice(-10) // Last 10 versions
      }));
      return;
    }
    targetVersion = version;
  }

  const tarballUrl = metadata.versions[targetVersion].dist.tarball;
  console.log(`Fetching tarball for ${packageName}@${targetVersion}`);

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

      const redirectOptions = redirectUrl.startsWith('http')
        ? redirectUrl
        : {
            hostname: ARTIFACT_REGISTRY,
            path: redirectUrl,
            headers: {
              'Host': ARTIFACT_REGISTRY,
              'Authorization': `Bearer ${token}`
            }
          };

      https.get(redirectOptions, (finalRes) => {
        // Add version header
        const headers = {
          'Content-Type': finalRes.headers['content-type'] || 'application/octet-stream',
          'Content-Length': finalRes.headers['content-length'],
          'X-Package-Version': targetVersion,
          'Content-Disposition': `attachment; filename="${packageName.replace('@', '').replace('/', '-')}-${targetVersion}.tgz"`
        };
        res.writeHead(finalRes.statusCode, headers);
        finalRes.pipe(res);
      }).on('error', (err) => {
        console.error('Redirect fetch error:', err);
        res.writeHead(502, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'Failed to fetch tarball from redirect', details: err.message }));
      });
    } else {
      // Direct response
      const headers = {
        ...proxyRes.headers,
        'X-Package-Version': targetVersion,
        'Content-Disposition': `attachment; filename="${packageName.replace('@', '').replace('/', '-')}-${targetVersion}.tgz"`
      };
      res.writeHead(proxyRes.statusCode, headers);
      proxyRes.pipe(res);
    }
  });

  proxyReq.on('error', (err) => {
    console.error('Tarball fetch error:', err);
    res.writeHead(502, { 'Content-Type': 'application/json' });
    res.end(JSON.stringify({ error: 'Failed to fetch tarball', details: err.message }));
  });
}

// Proxy server
const server = http.createServer(async (req, res) => {
  // Enable CORS
  res.setHeader('Access-Control-Allow-Origin', '*');
  res.setHeader('Access-Control-Allow-Methods', 'GET, OPTIONS');
  res.setHeader('Access-Control-Allow-Headers', 'Content-Type');

  if (req.method === 'OPTIONS') {
    res.writeHead(204);
    res.end();
    return;
  }

  // Health check
  if (req.url === '/health') {
    res.writeHead(200, { 'Content-Type': 'text/plain' });
    res.end('healthy\n');
    return;
  }

  try {
    // Get access token
    const token = await getAccessToken();
    const parsedUrl = url.parse(req.url, true);
    const pathname = parsedUrl.pathname;
    const query = parsedUrl.query;

    // Handle /versions/{package} endpoint - list all available versions
    if (pathname.startsWith('/versions/')) {
      const packageName = pathname.substring(10); // Remove '/versions/'
      console.log(`Fetching versions for ${packageName}`);

      try {
        const metadata = await getPackageMetadata(packageName, token, query.refresh === 'true');
        const versions = Object.keys(metadata.versions).sort((a, b) => {
          // Sort by version (semver-ish)
          return a.localeCompare(b, undefined, { numeric: true, sensitivity: 'base' });
        });

        const response = {
          name: metadata.name,
          latest: metadata['dist-tags']['latest-dev'] || metadata['dist-tags']['latest'],
          'dist-tags': metadata['dist-tags'],
          versions: versions,
          totalVersions: versions.length
        };

        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify(response, null, 2));
      } catch (err) {
        res.writeHead(404, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'Package not found', details: err.message }));
      }
      return;
    }

    // Handle /{package} endpoint - get package metadata (like npm view)
    // Matches /@scope/package or /package
    const packageMetadataMatch = pathname.match(/^\/(@[^\/]+\/[^\/]+)$/);
    if (packageMetadataMatch) {
      const packageName = packageMetadataMatch[1];
      console.log(`Fetching metadata for ${packageName}`);

      try {
        const metadata = await getPackageMetadata(packageName, token, query.refresh === 'true');
        const latestVersion = metadata['dist-tags']['latest-dev'] || metadata['dist-tags']['latest'];
        const latestInfo = metadata.versions[latestVersion];

        const response = {
          name: metadata.name,
          version: latestVersion,
          description: metadata.description,
          'dist-tags': metadata['dist-tags'],
          versions: Object.keys(metadata.versions),
          latest: {
            version: latestVersion,
            tarball: `https://pkg.alkanes.build/dist/${packageName}?v=${latestVersion}`,
            shasum: latestInfo?.dist?.shasum,
            integrity: latestInfo?.dist?.integrity
          },
          install: {
            npm: `npm install --save-dev https://pkg.alkanes.build/dist/${packageName}?v=${latestVersion}`,
            pnpm: `pnpm install --save-dev https://pkg.alkanes.build/dist/${packageName}?v=${latestVersion}`,
            yarn: `yarn add -D https://pkg.alkanes.build/dist/${packageName}?v=${latestVersion}`
          },
          repository: metadata.repository,
          homepage: metadata.homepage,
          license: metadata.license
        };

        res.writeHead(200, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify(response, null, 2));
      } catch (err) {
        res.writeHead(404, { 'Content-Type': 'application/json' });
        res.end(JSON.stringify({ error: 'Package not found', details: err.message }));
      }
      return;
    }

    // Handle /dist/{package} endpoint - direct tarball download with optional ?v=version
    if (pathname.startsWith('/dist/')) {
      const packageName = pathname.substring(6); // Remove '/dist/'
      const version = query.v || query.version;

      await fetchTarball(res, packageName, version, token);
      return;
    }

    // Default: proxy request to Artifact Registry (npm registry compatibility)
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
  console.log('');
  console.log('Endpoints:');
  console.log('  GET /@scope/package          - Package metadata and latest version');
  console.log('  GET /versions/@scope/package - List all available versions');
  console.log('  GET /dist/@scope/package     - Download latest tarball');
  console.log('  GET /dist/@scope/package?v=X - Download specific version tarball');
  console.log('  GET /health                  - Health check');
});
