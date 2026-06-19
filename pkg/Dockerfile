FROM node:20-alpine

# Create app directory
WORKDIR /app

# Copy package files and install dependencies
COPY package.json ./
RUN npm install --production

COPY server.js ./

# Cloud Run requires listening on PORT environment variable
ENV PORT=8080
EXPOSE 8080

# Run the proxy
CMD ["node", "server.js"]
