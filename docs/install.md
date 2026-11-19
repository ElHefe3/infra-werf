# Install InfraWerf

This page covers installing the Windows desktop app. To build it yourself, see [Build from source](build.md).

## Requirements

| Requirement | Notes |
| --- | --- |
| Windows 11, 64-bit | The release target. Other platforms are not supported yet. |
| Microsoft Edge WebView2 Runtime | Already present on most Windows 11 PCs. If missing, the installer downloads it, so it needs internet access once. |
| OpenSSH client | Windows' built-in OpenSSH (`ssh.exe`) with your servers configured in `%USERPROFILE%\.ssh\config`, or the equivalent for your setup. |
| Your existing SSH access | Public-key login to each server you want to manage. InfraWerf does not create keys or accounts for you. |

You do not need administrator rights to install.

## Install

1. Get the installer, named `InfraWerf_<version>_x64-setup.exe`, from the release you were given. Keep the file you download; you can verify it later with its SHA-256 hash.
2. Run it. It installs **for your user only** under `%LOCALAPPDATA%\InfraWerf`.
3. Open **InfraWerf** from the Start menu.

### Windows SmartScreen warning

The current installer is **not code-signed yet**, so Windows may show *"Windows protected your PC"*. Check that the file is the one you intended (compare the SHA-256 with the value in the release notes), then choose **More info → Run anyway**. Do not disable SmartScreen to get past it.

To check the hash, open PowerShell in the download folder:

```powershell
Get-FileHash .\InfraWerf_<version>_x64-setup.exe -Algorithm SHA256
```

A matching hash shows the file is unchanged. It does not prove who published it.

## First launch

On first launch InfraWerf creates an empty workspace in its data folder. Nothing is connected or scanned automatically: no server is contacted until you add one and ask for an action.

Continue with the [user guide](user-guide.md#first-steps).

## Where things are

| What | Location |
| --- | --- |
| Application files | `%LOCALAPPDATA%\InfraWerf` |
| Your saved workspace (map, inventory, settings) | `%APPDATA%\dev.atlas.workspace\atlas.sqlite3` |
| Plugins folder | Shown in **Settings → Plugins** |

The data folder keeps the original `dev.atlas.workspace` name for compatibility with earlier builds. It is not a mistake.

## Update

1. Close InfraWerf completely. If it is still listed in Task Manager after you close the window, end it there before installing.
2. Back up your data: see [Data and backup](data-and-backup.md#back-up).
3. Run the new `InfraWerf_<version>_x64-setup.exe`.

Updating replaces the application files only. Your data folder is left alone, and your saved map and servers carry over.

## Uninstall

1. Close InfraWerf.
2. Open **Settings → Apps → Installed apps**, find **InfraWerf**, and choose **Uninstall**. You can also run `%LOCALAPPDATA%\InfraWerf\uninstall.exe`.
3. The uninstaller asks whether to **delete app data**. Leave it **unchecked** unless you are sure you want to erase your map, inventory and plugin records. Back up first if you might want them again.

## Next steps

- [User guide](user-guide.md): what each part of the app does.
- [Security model](security.md): what InfraWerf will and will not do with your servers.
- [Troubleshooting](troubleshooting.md): if something does not work.
