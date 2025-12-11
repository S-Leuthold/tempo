# Privacy Policy - Trainer Log

**Last Updated:** December 11, 2024

## Overview

Trainer Log is a personal training analysis application that combines data from Strava and Oura Ring to provide coaching insights. This app is designed for personal use only.

## Data Collection

We collect and process:
- **Strava Data:** Workout activities (pace, power, heart rate, duration)
- **Oura Data:** Sleep duration, sleep stages (deep/REM/light), heart rate variability (HRV), resting heart rate
- **Computed Metrics:** Training stress balance (TSB), acute/chronic training load, intensity distribution

## Data Storage

All data is stored **locally on your device** in a SQLite database located at:
- macOS: `~/Library/Application Support/com.trainer-log.dev/trainer-log.db`

**We do NOT:**
- Send your data to external servers (except to fetch from Strava/Oura APIs)
- Share your data with third parties
- Use your data for any purpose other than generating your personal coaching analysis

## Data Usage

Your data is used exclusively to:
- Analyze workout trends
- Assess recovery status
- Generate training recommendations
- Display progress over time

Analysis is performed locally using Claude AI API (Anthropic) for natural language generation. Only aggregated metrics are sent to Claude, not raw activity details.

## Third-Party Services

We integrate with:
- **Strava API** - To fetch your workout data
- **Oura API** - To fetch your sleep and recovery data
- **Claude AI (Anthropic)** - To generate coaching insights from computed metrics

Each service has its own privacy policy:
- [Strava Privacy Policy](https://www.strava.com/legal/privacy)
- [Oura Privacy Policy](https://ouraring.com/privacy-policy)
- [Anthropic Privacy Policy](https://www.anthropic.com/legal/privacy)

## Your Rights

You can:
- **Disconnect** Strava or Oura at any time (revokes access)
- **Delete** all stored data by deleting the application
- **Export** your data (SQLite database is readable)
- **Request** information about what data is stored

## Data Retention

Data is retained locally until you:
- Disconnect a service (future workouts won't sync)
- Uninstall the application (all data deleted)

## Security

- OAuth tokens stored locally with operating system encryption
- No data transmitted except to authorized APIs (Strava, Oura, Claude)
- Local database not encrypted (standard SQLite file)

## Changes to Privacy Policy

We may update this privacy policy. Changes will be reflected in the app and this document.

## Contact

Questions about privacy: sam.leuthold@gmail.com
