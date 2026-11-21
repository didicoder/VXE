/**
 * 添加设备扫描 store：扫描候选、列协议、试读、保存。
 * 所有 Tauri 调用走 lib/ipc；组件只消费本 store。
 */

import { defineStore } from 'pinia'
import { ref } from 'vue'
import * as ipc from '../lib/ipc'
import type { DeviceConfig } from '../types/device'
import type { DetectResult, DiscoveredDevice, ProfileChoice } from '../types/discover'
import { useConfigStore } from './config'

export const useDiscoverStore = defineStore('discover', () => {
  const devices = ref<DiscoveredDevice[]>([])
  const profiles = ref<ProfileChoice[]>([])
  const scanning = ref(false)
  const detecting = ref(false)
  const error = ref<string | null>(null)

  async function scan(): Promise<void> {
    scanning.value = true
    error.value = null
    try {
      devices.value = await ipc.scanDevices()
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
    } finally {
      scanning.value = false
    }
  }

  async function loadProfiles(): Promise<void> {
    try {
      profiles.value = await ipc.getProfileChoices()
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
    }
  }

  async function detect(
    device: Partial<DeviceConfig>,
    profileId = '',
  ): Promise<DetectResult | null> {
    detecting.value = true
    error.value = null
    try {
      return await ipc.detectDevice(device, profileId)
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
      return null
    } finally {
      detecting.value = false
    }
  }

  async function add(device: DiscoveredDevice): Promise<DeviceConfig | null> {
    error.value = null
    try {
      const added = await ipc.addDiscoveredDevice(device)
      await useConfigStore().load()
      return added
    } catch (cause) {
      error.value = ipc.errorMessage(cause)
      return null
    }
  }

  return {
    devices,
    profiles,
    scanning,
    detecting,
    error,
    scan,
    loadProfiles,
    detect,
    add,
  }
})
