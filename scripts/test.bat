@echo off
:: A1 — Full test suite  v2.8.0
:: Double-click or run from a terminal to execute all tests.
:: Requires PowerShell 5.1 or later (included in Windows 10/11).

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0test.ps1" %*
