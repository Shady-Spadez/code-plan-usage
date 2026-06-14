@echo off
REM Build the Coding Plan Widget MSI installer.
REM Requires WiX Toolset v3 or v4: https://wixtoolset.org/
REM
REM Usage:
REM   1. Build the release exe first: cargo build --release
REM   2. Run this script: build.bat
REM   3. The installer will be created as coding-plan-widget-installer.msi

candle.exe coding-plan-widget.wxs -arch x64
light.exe coding-plan-widget.wixobj -out coding-plan-widget-installer.msi
