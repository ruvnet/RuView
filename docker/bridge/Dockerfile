FROM python:3.12-slim

WORKDIR /app

COPY requirements.txt /app/requirements.txt
RUN pip install --no-cache-dir -r requirements.txt

COPY bridge.py /app/bridge.py

# Make sure both files are world-readable. Without this, a host umask of
# 077 (common on locked-down NAS exports) propagates through `COPY` and
# leaves /app/bridge.py with 0600 owned by root — unreadable by `nobody`.
RUN chmod 0644 /app/bridge.py /app/requirements.txt

ENV PYTHONUNBUFFERED=1
USER nobody

ENTRYPOINT ["python", "/app/bridge.py"]
