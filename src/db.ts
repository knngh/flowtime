import Database from '@tauri-apps/plugin-sql';
import type { Project, Task } from './types';

let db: Database | null = null;

async function getDb(): Promise<Database> {
  if (!db) {
    db = await Database.load('sqlite:flowtime.db');
  }
  return db;
}

function parseTask(row: Record<string, unknown>): Task {
  return {
    ...row,
    tags: typeof row.tags === 'string' ? JSON.parse(row.tags as string) : (row.tags as string[]),
  } as unknown as Task;
}

// ── Projects ──

export async function getProjects(): Promise<Project[]> {
  const database = await getDb();
  return await database.select<Project[]>('SELECT * FROM projects ORDER BY created_at ASC');
}

export async function createProject(name: string, color: string = '#3B82F6'): Promise<Project> {
  const database = await getDb();
  const id = crypto.randomUUID();
  await database.execute(
    'INSERT INTO projects (id, name, color) VALUES ($1, $2, $3)',
    [id, name, color],
  );
  return { id, name, color, created_at: '', updated_at: '' };
}

export async function renameProject(id: string, name: string): Promise<void> {
  const database = await getDb();
  await database.execute(
    "UPDATE projects SET name = $1, updated_at = datetime('now') WHERE id = $2",
    [name, id],
  );
}

export async function deleteProject(id: string): Promise<void> {
  const database = await getDb();
  await database.execute('DELETE FROM projects WHERE id = $1', [id]);
}

// ── Tasks ──

export async function getTasks(projectId?: string | null): Promise<Task[]> {
  const database = await getDb();
  const sql = projectId
    ? 'SELECT * FROM tasks WHERE project_id = $1 ORDER BY scheduled_start ASC, created_at DESC'
    : 'SELECT * FROM tasks ORDER BY scheduled_start ASC, created_at DESC';
  const rows = projectId
    ? await database.select<Record<string, unknown>[]>(sql, [projectId])
    : await database.select<Record<string, unknown>[]>(sql);
  return rows.map(parseTask);
}

export async function createTask(
  title: string,
  projectId: string | null = null,
  priority: Task['priority'] = 'B',
  estimatedDurationMin: number = 30,
): Promise<Task> {
  const database = await getDb();
  const id = crypto.randomUUID();
  await database.execute(
    `INSERT INTO tasks (id, title, project_id, priority, estimated_duration_min)
     VALUES ($1, $2, $3, $4, $5)`,
    [id, title, projectId, priority, estimatedDurationMin],
  );
  return {
    id, title, priority, estimated_duration_min: estimatedDurationMin,
    source: 'manual', source_url: null, project_id: projectId,
    tags: [], status: 'todo',
    scheduled_start: null, scheduled_end: null,
    actual_start: null, actual_end: null,
    created_at: '', updated_at: '',
  };
}

export async function updateTask(
  id: string,
  fields: Partial<Pick<Task, 'title' | 'priority' | 'estimated_duration_min' | 'project_id' | 'status'>>,
): Promise<void> {
  const database = await getDb();
  const sets: string[] = [];
  const values: (string | number)[] = [];
  let idx = 1;

  for (const [key, value] of Object.entries(fields)) {
    if (value !== undefined) {
      sets.push(`${key} = $${idx}`);
      values.push(value as string | number);
      idx++;
    }
  }

  if (sets.length === 0) return;
  sets.push(`updated_at = datetime('now')`);
  values.push(id);

  await database.execute(
    `UPDATE tasks SET ${sets.join(', ')} WHERE id = $${idx}`,
    values,
  );
}

export async function deleteTask(id: string): Promise<void> {
  const database = await getDb();
  await database.execute('DELETE FROM tasks WHERE id = $1', [id]);
}

export async function refreshCache() {
  db = null;
}
