/**
 * @file config_api.h
 * @brief Remote configuration over HTTP -- change node settings without USB.
 *
 * Deliberately NOT called "OTA". OTA in this firmware means a firmware image
 * written to an app partition by ota_update.c. This is provisioning: the same
 * parameters provision.py writes over USB, writable over the network instead,
 * on the same authenticated server and behind the same PSK.
 *
 * The parameters split by whether a bad value can ORPHAN a node:
 *
 *   recoverable  A wrong target_ip sends CSI nowhere, but the node keeps its
 *                association and stays reachable on :8032, so the next config
 *                push fixes it. Applied immediately.
 *
 *   orphaning    A wrong ssid, password or channel means the node never
 *                rejoins the network and no push can ever reach it again --
 *                strictly worse than a bad OTA, which at least has a second
 *                partition to fall back to. These go through a trial: the old
 *                values are banked, the new ones are written, and the node
 *                reboots. It commits only on proving it can still associate;
 *                otherwise it restores the bank and reboots itself.
 *
 * That trial is the app-level equivalent of bootloader rollback, and unlike
 * bootloader rollback it can itself be delivered by OTA.
 */
#pragma once

#include "esp_err.h"
#include "esp_http_server.h"

/** Register /config and /config/trial on an already-running server. */
esp_err_t config_api_register(httpd_handle_t server);

/**
 * Arm the revert timer if a config trial is pending.
 * Call early in app_main(), before WiFi starts.
 */
void config_trial_boot_check(void);

/**
 * Report a successful association, committing any pending trial.
 * Call from the IP_EVENT_STA_GOT_IP handler.
 */
void config_trial_notify_connected(void);
