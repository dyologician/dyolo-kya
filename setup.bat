@echo off
:: A1 — Know Your Agent  v2.8.0
:: Double-click this file to start A1 on Windows.
:: Requires PowerShell 5.1 or later (included in Windows 10/11).

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\setup.ps1" %*
