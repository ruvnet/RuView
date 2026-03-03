FROM node:20-alpine

WORKDIR /app

# Copy package files
COPY package*.json ./

# Install dependencies
RUN npm install

# Copy all benchmark files
COPY . .

# Create results directory
RUN mkdir -p results

# Set entrypoint
ENTRYPOINT ["npm", "run"]

# Default command
CMD ["benchmark:all"]