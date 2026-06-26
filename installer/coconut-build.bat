@echo off
REM Build the Coconut Plan Widget MSI installer.
REM Requires WiX Toolset v3 or v4: https://wixtoolset.org/
REM
REM Usage:
REM   1. Build the release exe first: cargo build --release --bin coding-plan-widget-coconut
REM   2. Run this script: coconut-build.bat
REM   3. The installer will be created as coconut-plan-widget.msi

candle.exe coconut-plan-widget.wxs -arch x64
light.exe coconut-plan-widget.wixobj -out coconut-plan-widget.msi
