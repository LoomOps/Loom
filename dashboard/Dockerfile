ARG BASE=python:3.12-slim
FROM ${BASE}
WORKDIR /opt/loom-dashboard
COPY LICENSE THIRD_PARTY_NOTICES.md /usr/share/doc/loom/
COPY requirements.txt /opt/loom-dashboard/
RUN pip install --no-cache-dir -r requirements.txt
COPY build-info.json build_version.py /opt/loom-dashboard/
COPY ssh_access.py server.py sso.py admission.py managed_resources.py sharing.py admission_http.py gateway.py gateway.js gateway.html gateway.css /opt/loom-dashboard/
COPY branding/ /opt/loom-dashboard/branding/
COPY theme/ /opt/loom-dashboard/theme/
COPY public/ /opt/loom-dashboard/public/
ENV PYTHONDONTWRITEBYTECODE=1 PYTHONUNBUFFERED=1 LOOM_DASHBOARD_BIND=0.0.0.0
USER 65534:65534
EXPOSE 8080
HEALTHCHECK --interval=15s --timeout=3s CMD ["python3","-c","from urllib.request import urlopen; urlopen('http://127.0.0.1:8080/healthz',timeout=2).close()"]
ENTRYPOINT ["python3","-B","/opt/loom-dashboard/server.py"]
