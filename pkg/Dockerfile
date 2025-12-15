FROM nginx:alpine

# Copy nginx configuration
COPY nginx.conf /etc/nginx/nginx.conf

# Cloud Run requires listening on PORT environment variable
ENV PORT=8080
EXPOSE 8080

CMD ["nginx", "-g", "daemon off;"]
