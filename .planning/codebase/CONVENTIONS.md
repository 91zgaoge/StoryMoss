# Coding Conventions

## Overview
This document outlines the coding standards and conventions used in the CINEMA-AI frontend codebase. The project uses TypeScript, React, and modern tooling (ESLint, Prettier, Tailwind CSS).

## File Organization

### Directory Structure
```
src/
├── components/          # React components (organized by feature)
├── pages/              # Page-level components
├── hooks/              # Custom React hooks
├── services/           # API and backend communication services
├── stores/             # State management (Zustand)
├── types/              # TypeScript type definitions
├── utils/              # Utility functions and helpers
├── test/               # Test configuration and setup
└── frontstage/         # Frontend-specific UI components
```

### File Naming
- **Components**: PascalCase (e.g., `ErrorBoundary.tsx`, `BookDetailView.tsx`)
- **Hooks**: camelCase with `use` prefix (e.g., `useSyncStore.ts`, `useAiOperations.ts`)
- **Services**: camelCase (e.g., `settings.ts`, `modelService.ts`)
- **Utilities**: camelCase (e.g., `logger.ts`, `errorHandler.ts`)
- **Types**: camelCase (e.g., `book-deconstruction.ts`, `collab.ts`)
- **Tests**: `*.test.ts` or `*.test.tsx` suffix

## TypeScript Conventions

### Type Definitions
- Use `interface` for object shapes, especially for component props and API responses
- Use `type` for unions, primitives, and complex type compositions
- Always export types that are used across modules

```typescript
// Good: Interface for component props
interface BookDetailViewProps {
  analysis: BookAnalysisResult;
  onConvertToStory: () => void;
  isConverting: boolean;
}

// Good: Type for union
type TabType = 'overview' | 'characters' | 'chapters' | 'story-arc';

// Good: Type for complex composition
type LogLevel = 'debug' | 'info' | 'warn' | 'error';
```

### Naming Conventions
- **Interfaces**: Suffix with `Props` for component props, `Options` for function options, `State` for state objects
- **Types**: Use descriptive names that indicate the data structure
- **Enums**: Use PascalCase (though prefer union types for better tree-shaking)
- **Constants**: UPPER_SNAKE_CASE for module-level constants

```typescript
// Good
interface AppState {
  currentView: ViewType;
  setCurrentView: (view: ViewType) => void;
}

interface SyncStoreOptions {
  onStoryCreated?: (storyId: string, title?: string) => void;
}

const STORAGE_KEY = 'storyforge:log:config';
const LEVEL_ORDER: Record<LogLevel, number> = { ... };
```

### Imports
- Use absolute imports with `@/` alias for project files
- Group imports: external libraries, then internal modules
- Use named imports for specific exports, default imports for modules

```typescript
import { Component, type ReactNode } from 'react';
import { invoke } from '@tauri-apps/api/core';
import type { Story, Character } from '@/types/index';
import { createLogger } from '@/utils/logger';
```

## React Conventions

### Component Structure
- Functional components are the default
- Use class components only for error boundaries
- Props should be defined as interfaces
- Destructure props in function parameters

```typescript
interface BookUploadPanelProps {
  onUpload: (filePath: string) => void;
  isUploading: boolean;
}

export function BookUploadPanel({ onUpload, isUploading }: BookUploadPanelProps) {
  // component body
}
```

### Hooks
- Custom hooks follow the `useXxx` naming convention
- Hooks should be placed in `src/hooks/` directory
- Complex hooks should include JSDoc comments explaining their purpose
- Use `useRef` for mutable values that don't trigger re-renders

```typescript
export function useSyncStore(options: SyncStoreOptions = {}) {
  const queryClient = useQueryClient();
  const optionsRef = useRef(options);
  optionsRef.current = options;
  // hook implementation
}
```

### State Management
- Use Zustand for global state (see `appStore.ts`)
- Use React Query for server state and caching
- Use local `useState` for component-level UI state
- Store state follows the pattern: state properties, then setter functions

```typescript
interface AppState {
  // Data
  stories: Story[];
  setStories: (stories: Story[]) => void;
  addStory: (story: Story) => void;
  
  // Loading States
  isLoading: boolean;
  setIsLoading: (loading: boolean) => void;
}
```

### Event Handlers
- Prefix handler functions with `on` (e.g., `onCancel`, `onSelect`)
- Use `useCallback` for handlers passed to child components
- Handlers should be typed with proper event types

```typescript
const handleDrop = useCallback(
  (e: React.DragEvent) => {
    e.preventDefault();
    const file = e.dataTransfer.files[0];
    if (file) handleFile(file);
  },
  [onUpload]
);
```

## Styling Conventions

### Tailwind CSS
- Use Tailwind utility classes for styling
- Avoid inline styles; use `cn()` utility for conditional classes
- Use the `cn()` function from `@/utils/cn` for merging Tailwind classes

```typescript
import { cn } from '@/utils/cn';

// Good: Using cn() for conditional classes
<div className={cn('base-class', isActive && 'active-class')}>
  Content
</div>

// Good: Merging conflicting Tailwind classes
const result = cn('px-2 py-1', 'px-4'); // resolves to 'py-1 px-4'
```

### Color Scheme
- Use custom color tokens defined in Tailwind config (e.g., `cinema-950`, `cinema-gold`)
- Maintain consistency with the design system

## Logging Conventions

### Logger Usage
- Use `createLogger()` to create module-specific loggers
- Logger namespace format: `domain:module` (e.g., `ui:App`, `api:tauri`, `services:settings`)
- Log levels: `debug`, `info`, `warn`, `error`
- Warn and error logs are automatically synced to backend

```typescript
import { createLogger } from '@/utils/logger';

const logger = createLogger('ui:MyComponent');
logger.debug('Debug message');
logger.info('Info message');
logger.warn('Warning message', { context: 'data' });
logger.error('Error message', { error: e });
```

### Predefined Loggers
- `apiLogger`: API and Tauri communication
- `uiLogger`: UI and component events
- `aiLogger`: AI engine operations
- `syncLogger`: State synchronization
- `wsLogger`: WebSocket and collaboration

## Error Handling Conventions

### Error Handling Patterns
- Use try-catch for async operations
- Provide meaningful error messages
- Log errors with context information
- Use the `handleError()` utility for consistent error handling

```typescript
try {
  const result = await someAsyncOperation();
} catch (e) {
  logger.error('Operation failed', { error: e });
  handleError(e, { 
    context: 'MyComponent',
    showToast: true 
  });
}
```

### Error Boundary
- Wrap top-level components with `ErrorBoundary`
- Error boundaries catch React rendering errors
- Logged errors include component stack information

```typescript
import { ErrorBoundary } from '@/components/ErrorBoundary';

<ErrorBoundary>
  <YourComponent />
</ErrorBoundary>
```

## Service Layer Conventions

### Service Functions
- Services handle API communication and backend integration
- Use `invoke()` from Tauri for IPC calls
- Provide fallback implementations for browser environments
- Export async functions that return typed data

```typescript
export async function getSettings(): Promise<AppSettings> {
  try {
    return await invoke<AppSettings>('get_settings');
  } catch (e) {
    const isTauri = !!(window as any).__TAURI__;
    if (!isTauri) {
      return BROWSER_FALLBACK_SETTINGS;
    }
    throw e;
  }
}
```

### Service Organization
- Group related functions in a single service file
- Use consistent naming: `get*`, `create*`, `update*`, `delete*`, `fetch*`
- Include JSDoc comments for public functions

## Code Style

### Formatting
- Use Prettier for automatic formatting
- Line length: 100 characters (configured in Prettier)
- Indentation: 2 spaces
- Semicolons: required
- Quotes: single quotes for strings (except JSX attributes)

### Naming Patterns
- **Boolean variables**: Prefix with `is`, `has`, `can`, `should` (e.g., `isLoading`, `hasError`, `canDelete`)
- **Event callbacks**: Prefix with `on` (e.g., `onStoryCreated`, `onCharacterUpdated`)
- **Getter functions**: Prefix with `get` (e.g., `getSettings`, `getModels`)
- **Setter functions**: Prefix with `set` (e.g., `setCurrentView`, `setError`)
- **Utility functions**: Descriptive verb-noun pattern (e.g., `formatDate`, `countWords`, `truncateText`)

### Comments
- Use JSDoc for public functions and complex logic
- Avoid obvious comments; focus on "why" not "what"
- Use section comments for logical groupings (e.g., `// ---- Query Key Constants ----`)

```typescript
/**
 * 统一实时状态同步中心 Hook
 *
 * 监听后端 `sync-event` 事件，自动刷新 TanStack Query 缓存，
 * 实现前后台数据自动对齐。
 */
export function useSyncStore(options: SyncStoreOptions = {}) {
  // implementation
}
```

## Async/Await Conventions

- Prefer async/await over `.then()` chains
- Always handle errors in async functions
- Use `useEffect` cleanup functions for subscriptions

```typescript
useEffect(() => {
  let unlisten: UnlistenFn | undefined;

  const setup = async () => {
    unlisten = await listen('sync-event', (event) => {
      // handle event
    });
  };

  setup();
  return () => {
    if (unlisten) unlisten();
  };
}, [queryClient]);
```

## Constants and Configuration

- Define constants at module level, not inside functions
- Use `Record<K, V>` for mapping objects
- Group related constants together with section comments

```typescript
const KEYS = {
  stories: ['stories'],
  characters: (storyId?: string) => storyId ? ['characters', storyId] : ['characters'],
  scenes: (storyId?: string) => storyId ? ['scenes', storyId] : ['scenes'],
};

const LEVEL_ORDER: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};
```

## Accessibility

- Use semantic HTML elements
- Include proper ARIA labels for interactive elements
- Ensure keyboard navigation support
- Test with screen readers for critical components

## Performance Considerations

- Use `useCallback` for event handlers passed to child components
- Use `useMemo` for expensive computations
- Leverage React Query for efficient data fetching and caching
- Avoid unnecessary re-renders with proper dependency arrays

## ESLint and Prettier

- ESLint configuration enforces code quality rules
- Prettier handles automatic code formatting
- Run `npm run lint` to check for issues
- Run `npm run format` to auto-fix formatting issues
- Pre-commit hooks ensure code quality before commits
