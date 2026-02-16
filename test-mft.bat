@echo off
cd /d "%~dp0"
echo Testing MFT Scanner with Administrator privileges
echo.
target\release\debug-scan.exe C:\
echo.
pause
