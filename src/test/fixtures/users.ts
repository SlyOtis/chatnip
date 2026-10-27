import type { User } from '$lib/tauri/types'

export const userAlice: User = {
  id: 'user-001',
  email: 'alice@example.com',
  phone: '+1234567890',
  display_name: 'Alice Johnson',
  first_name: 'Alice',
  last_name: 'Johnson',
  avatar_url: 'https://example.com/avatars/alice.png',
  avatar_data: null,
  job_title: 'Software Engineer',
  department: 'Engineering',
  company: 'Example Corp',
  office_location: 'Building A',
  first_seen: '2025-01-15T10:00:00Z',
  last_updated: '2025-06-01T12:00:00Z',
}

export const userBob: User = {
  id: 'user-002',
  email: 'bob@example.com',
  phone: null,
  display_name: 'Bob Smith',
  first_name: 'Bob',
  last_name: 'Smith',
  avatar_url: null,
  avatar_data: null,
  job_title: 'Product Manager',
  department: 'Product',
  company: 'Example Corp',
  office_location: null,
  first_seen: '2025-02-10T09:00:00Z',
  last_updated: '2025-06-01T12:00:00Z',
}

export const userSystem: User = {
  id: 'user-system',
  email: null,
  phone: null,
  display_name: 'System',
  first_name: null,
  last_name: null,
  avatar_url: null,
  avatar_data: null,
  job_title: null,
  department: null,
  company: null,
  office_location: null,
  first_seen: '2025-01-01T00:00:00Z',
  last_updated: '2025-01-01T00:00:00Z',
}
