{{/*
Expand the name of the chart.
*/}}
{{- define "open-tappd.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "open-tappd.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "open-tappd.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "open-tappd.labels" -}}
helm.sh/chart: {{ include "open-tappd.chart" . }}
{{ include "open-tappd.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "open-tappd.selectorLabels" -}}
app.kubernetes.io/name: {{ include "open-tappd.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "open-tappd.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "open-tappd.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
App secret name — either existing or generated
*/}}
{{- define "open-tappd.secretName" -}}
{{- if .Values.secrets.existingSecret }}
{{- .Values.secrets.existingSecret }}
{{- else }}
{{- include "open-tappd.fullname" . }}
{{- end }}
{{- end }}

{{/*
Database host — built-in postgresql or external
*/}}
{{- define "open-tappd.databaseHost" -}}
{{- if .Values.postgresql.enabled }}
{{- printf "%s-postgresql" .Release.Name }}
{{- else }}
{{- required "database.host is required when postgresql.enabled is false" .Values.database.host }}
{{- end }}
{{- end }}

{{/*
Database URL
*/}}
{{- define "open-tappd.databaseUrl" -}}
{{- printf "postgres://%s:%s@%s:%d/%s"
    .Values.database.user
    "$(DB_PASSWORD)"
    (include "open-tappd.databaseHost" .)
    (int .Values.database.port)
    .Values.database.name
}}
{{- end }}

{{/*
Database password secret name
*/}}
{{- define "open-tappd.dbPasswordSecretName" -}}
{{- if .Values.database.existingPasswordSecret }}
{{- .Values.database.existingPasswordSecret }}
{{- else }}
{{- include "open-tappd.fullname" . }}-db
{{- end }}
{{- end }}

{{/*
Database password secret key
*/}}
{{- define "open-tappd.dbPasswordSecretKey" -}}
{{- if .Values.database.existingPasswordSecret }}
{{- .Values.database.existingPasswordSecretKey }}
{{- else }}
{{- "password" }}
{{- end }}
{{- end }}

{{/*
Database admin password secret name (for init setup)
*/}}
{{- define "open-tappd.dbAdminSecretName" -}}
{{- if .Values.database.setup.existingAdminSecret }}
{{- .Values.database.setup.existingAdminSecret }}
{{- else }}
{{- include "open-tappd.fullname" . }}-db
{{- end }}
{{- end }}

{{/*
Database admin password secret key
*/}}
{{- define "open-tappd.dbAdminSecretKey" -}}
{{- if .Values.database.setup.existingAdminSecret }}
{{- .Values.database.setup.existingAdminSecretKey }}
{{- else }}
{{- "admin-password" }}
{{- end }}
{{- end }}
