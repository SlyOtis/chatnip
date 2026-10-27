import { describe, expect, it, vi } from 'vitest'

vi.mock('$lib/tauri/api', () => ({ getAppSettings: vi.fn(), putAppSetting: vi.fn(), getSidebarLayout: vi.fn(), putSidebarLayout: vi.fn() }))

import { insertSidebarDivider, insertSidebarFolder, insertSidebarHeading, moveSidebarEntry, putSidebarEntry, removeSidebarEntry } from './layoutSettings'
import type { SidebarLayout } from '$lib/tauri/types'

const item = (key: string) => ({ kind: 'item' as const, key })
const base = (): SidebarLayout => ({ top: [item('notifications'), item('inbox'), item('done')], footer: [item('actions'), item('calendar')] })

describe('sidebar layout editing', () => {
  it('moves entries within a zone and across to the footer', () => {
    const down = moveSidebarEntry(base(), { zone: 'top', index: 0 }, { zone: 'top', index: 3 })
    expect(down.top.map((e) => (e.kind === 'item' ? e.key : e.kind))).toEqual(['inbox', 'done', 'notifications'])
    const footer = moveSidebarEntry(base(), { zone: 'top', index: 2 }, { zone: 'footer', index: 0 })
    expect(footer.footer[0]).toEqual(item('done'))
    expect(footer.top).toHaveLength(2)
  })

  it('adds dividers and folders, moves items into a folder and removes a folder without losing its items', () => {
    let layout = insertSidebarDivider(base(), { zone: 'top', index: 1 })
    expect(layout.top[1].kind).toBe('divider')
    layout = insertSidebarFolder(layout, 'top', 'Mail', 'lucide:mail')
    const folderIndex = layout.top.length - 1
    layout = moveSidebarEntry(layout, { zone: 'top', index: 0 }, { zone: 'top', index: 0, folder: folderIndex })
    const folder = layout.top[folderIndex - 1]
    expect(folder.kind === 'folder' && folder.items).toEqual([item('notifications')])
    layout = putSidebarEntry(layout, { zone: 'top', index: folderIndex - 1 }, { collapsed: true, name: 'Post' })
    const renamed = layout.top[folderIndex - 1]
    expect(renamed.kind === 'folder' && [renamed.name, renamed.collapsed]).toEqual(['Post', true])
    layout = removeSidebarEntry(layout, { zone: 'top', index: folderIndex - 1 })
    expect(layout.top.some((e) => e.kind === 'item' && e.key === 'notifications')).toBe(true)
  })

  it('adds a section heading that holds nothing, renames it, keeps it out of folders and removes it', () => {
    let layout = insertSidebarFolder(insertSidebarHeading(base(), 'top', 'Apps'), 'top', 'Mail')
    const heading = layout.top[3]
    expect(heading).toMatchObject({ kind: 'heading', name: 'Apps' })
    expect('items' in heading).toBe(false)
    layout = putSidebarEntry(layout, { zone: 'top', index: 3 }, { name: 'Categories', collapsed: true })
    expect(layout.top[3]).toEqual({ kind: 'heading', id: heading.kind === 'heading' ? heading.id : '', name: 'Categories' })
    expect(moveSidebarEntry(layout, { zone: 'top', index: 3 }, { zone: 'top', index: 0, folder: 4 })).toBe(layout)
    expect(removeSidebarEntry(layout, { zone: 'top', index: 3 }).top.some((e) => e.kind === 'heading')).toBe(false)
  })

  it('never removes an item and never nests a folder', () => {
    expect(removeSidebarEntry(base(), { zone: 'top', index: 0 }).top).toHaveLength(3)
    let layout = insertSidebarFolder(insertSidebarFolder(base(), 'top', 'A'), 'top', 'B')
    const nested = moveSidebarEntry(layout, { zone: 'top', index: 4 }, { zone: 'top', index: 0, folder: 3 })
    expect(nested).toBe(layout)
  })
})
