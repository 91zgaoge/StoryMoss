# Testing Patterns and Framework

## Overview
This document describes the testing framework, patterns, and best practices used in the CINEMA-AI frontend codebase. The project uses Vitest as the test runner with React Testing Library for component testing.

## Test Framework Setup

### Testing Stack
- **Test Runner**: Vitest
- **Component Testing**: React Testing Library
- **Assertion Library**: Vitest's built-in `expect` (compatible with Jest)
- **Test Configuration**: `vitest.config.ts` in project root

### Test File Organization
- Test files are colocated with source files in `__tests__` directories
- Test file naming: `*.test.ts` or `*.test.tsx`
- Test setup file: `src/test/setup.ts`

```
src/
├── utils/
│   ├── cn.ts
│   └── __tests__/
│       └── cn.test.ts
├── hooks/
│   ├── useSyncStore.ts
│   └── __tests__/
│       └── useSyncStore.test.ts
└── test/
    └── setup.ts
```

## Test Setup

### Global Test Configuration
The `src/test/setup.ts` file configures the test environment:

```typescript
import '@testing-library/jest-dom';
```

This imports Jest DOM matchers for enhanced assertions on DOM elements.

### Running Tests
```bash
# Run all tests
npm test

# Run tests in watch mode
npm test -- --watch

# Run tests for a specific file
npm test -- cn.test.ts

# Run tests with coverage
npm test -- --coverage
```

## Unit Testing Patterns

### Utility Function Tests
Test utility functions with clear input/output expectations:

```typescript
import { describe, it, expect } from 'vitest';
import { cn } from '../cn';

describe('cn', () => {
  it('should merge tailwind classes correctly', () => {
    const result = cn('px-2 py-1', 'px-4');
    expect(result).toBe('py-1 px-4'); // tailwind-merge resolves px conflict
  });

  it('should handle conditional classes', () => {
    const isActive = true;
    const result = cn('base-class', isActive && 'active-class');
    expect(result).toContain('base-class');
    expect(result).toContain('active-class');
  });

  it('should filter out falsy values', () => {
    const result = cn('a', false, null, undefined, '', 'b');
    expect(result).toBe('a b');
  });

  it('should handle empty input', () => {
    expect(cn()).toBe('');
  });
});
```

### Test Structure
- Use `describe()` to group related tests
- Use `it()` for individual test cases
- Use descriptive test names that explain the behavior being tested
- Follow the Arrange-Act-Assert pattern

```typescript
describe('functionName', () => {
  it('should do something specific', () => {
    // Arrange: Set up test data
    const input = 'test value';
    
    // Act: Call the function
    const result = functionName(input);
    
    // Assert: Verify the result
    expect(result).toBe('expected value');
  });
});
```

## Component Testing Patterns

### Component Test Setup
Use React Testing Library for component testing:

```typescript
import { render, screen } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { MyComponent } from '../MyComponent';

describe('MyComponent', () => {
  it('should render with correct text', () => {
    render(<MyComponent title="Test" />);
    expect(screen.getByText('Test')).toBeInTheDocument();
  });
});
```

### Querying Elements
Prefer queries in this order (from React Testing Library docs):
1. `getByRole()` - Most accessible, recommended
2. `getByLabelText()` - For form inputs
3. `getByPlaceholderText()` - For inputs with placeholders
4. `getByText()` - For text content
5. `getByTestId()` - Last resort, use data-testid attribute

```typescript
// Good: Using getByRole
expect(screen.getByRole('button', { name: /submit/i })).toBeInTheDocument();

// Good: Using getByText for simple cases
expect(screen.getByText('Welcome')).toBeInTheDocument();

// Avoid: Using getByTestId unless necessary
expect(screen.getByTestId('my-component')).toBeInTheDocument();
```

### User Interactions
Test user interactions using `userEvent`:

```typescript
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

it('should handle click events', async () => {
  const handleClick = vi.fn();
  render(<button onClick={handleClick}>Click me</button>);
  
  const button = screen.getByRole('button', { name: /click me/i });
  await userEvent.click(button);
  
  expect(handleClick).toHaveBeenCalledOnce();
});
```

## Hook Testing Patterns

### Testing Custom Hooks
Use `renderHook` from React Testing Library for testing hooks:

```typescript
import { renderHook, act } from '@testing-library/react';
import { describe, it, expect } from 'vitest';
import { useSyncStore } from '../useSyncStore';

describe('useSyncStore', () => {
  it('should initialize with default options', () => {
    const { result } = renderHook(() => useSyncStore());
    
    // Hook is now available in result.current
    expect(result.current).toBeDefined();
  });

  it('should call callback when event is triggered', async () => {
    const onStoryCreated = vi.fn();
    const { result } = renderHook(() => 
      useSyncStore({ onStoryCreated })
    );
    
    // Simulate event and verify callback
    await act(async () => {
      // trigger event
    });
    
    expect(onStoryCreated).toHaveBeenCalled();
  });
});
```

### Async Hook Testing
Use `waitFor` for async operations:

```typescript
import { renderHook, waitFor } from '@testing-library/react';

it('should handle async operations', async () => {
  const { result } = renderHook(() => useAsyncData());
  
  // Wait for the hook to update
  await waitFor(() => {
    expect(result.current.data).toBeDefined();
  });
});
```

## Mocking Patterns

### Mocking Modules
Use `vi.mock()` to mock entire modules:

```typescript
import { vi, describe, it, expect } from 'vitest';
import { getSettings } from '../settings';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn().mockResolvedValue({ /* mock data */ })
}));

describe('settings service', () => {
  it('should fetch settings', async () => {
    const settings = await getSettings();
    expect(settings).toBeDefined();
  });
});
```

### Mocking Functions
Use `vi.fn()` to create mock functions:

```typescript
const mockCallback = vi.fn();
const mockCallback = vi.fn().mockReturnValue('value');
const mockCallback = vi.fn().mockResolvedValue({ data: 'value' });

// Verify calls
expect(mockCallback).toHaveBeenCalled();
expect(mockCallback).toHaveBeenCalledWith('arg1', 'arg2');
expect(mockCallback).toHaveBeenCalledOnce();
```

### Mocking React Query
For components using React Query, provide a mock QueryClient:

```typescript
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { render } from '@testing-library/react';

const createTestQueryClient = () => new QueryClient({
  defaultOptions: {
    queries: { retry: false },
    mutations: { retry: false },
  },
});

const wrapper = ({ children }) => (
  <QueryClientProvider client={createTestQueryClient()}>
    {children}
  </QueryClientProvider>
);

render(<MyComponent />, { wrapper });
```

## Assertion Patterns

### Common Assertions
```typescript
// Existence
expect(element).toBeInTheDocument();
expect(element).toExist();

// Visibility
expect(element).toBeVisible();
expect(element).toHaveClass('active');

// Content
expect(element).toHaveTextContent('text');
expect(element).toHaveValue('value');

// Attributes
expect(element).toHaveAttribute('href', '/path');
expect(element).toHaveAttribute('disabled');

// Functions
expect(mockFn).toHaveBeenCalled();
expect(mockFn).toHaveBeenCalledWith(arg);
expect(mockFn).toHaveBeenCalledTimes(2);

// Async
await expect(promise).resolves.toBe(value);
await expect(promise).rejects.toThrow();
```

## Test Coverage

### Coverage Goals
- Aim for >80% coverage on critical paths
- Focus on testing behavior, not implementation details
- Prioritize testing error cases and edge cases

### Running Coverage Reports
```bash
npm test -- --coverage
```

Coverage reports are generated in the `coverage/` directory.

## Best Practices

### Do's
- ✅ Test user behavior, not implementation details
- ✅ Use descriptive test names that explain what is being tested
- ✅ Keep tests focused and independent
- ✅ Mock external dependencies (API calls, timers, etc.)
- ✅ Use `beforeEach` and `afterEach` for setup and cleanup
- ✅ Test error cases and edge cases
- ✅ Use `vi.useFakeTimers()` for time-dependent code

### Don'ts
- ❌ Don't test implementation details (internal state, private methods)
- ❌ Don't create interdependent tests
- ❌ Don't use `setTimeout` in tests; use fake timers instead
- ❌ Don't test third-party libraries
- ❌ Don't write overly complex test logic
- ❌ Don't ignore test failures; fix them immediately

## Test Organization

### Test File Structure
```typescript
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest';
import { functionToTest } from '../module';

describe('Module Name', () => {
  // Setup
  beforeEach(() => {
    // Reset mocks and state
  });

  afterEach(() => {
    // Cleanup
  });

  describe('Feature/Function Name', () => {
    it('should do something', () => {
      // test
    });

    it('should handle edge case', () => {
      // test
    });
  });

  describe('Error Handling', () => {
    it('should throw on invalid input', () => {
      expect(() => functionToTest(null)).toThrow();
    });
  });
});
```

## Debugging Tests

### Debug Output
Use `screen.debug()` to print the DOM:

```typescript
import { render, screen } from '@testing-library/react';

it('should render correctly', () => {
  render(<MyComponent />);
  screen.debug(); // Prints the entire DOM
  screen.debug(screen.getByRole('button')); // Prints specific element
});
```

### Logging
Use `console.log()` for debugging (will appear in test output):

```typescript
it('should log values', () => {
  const result = functionToTest();
  console.log('Result:', result);
  expect(result).toBeDefined();
});
```

### Running Single Test
Use `.only` to run a single test:

```typescript
it.only('should run only this test', () => {
  // This test will run in isolation
});
```

## Continuous Integration

### Pre-commit Hooks
Tests are run automatically before commits via pre-commit hooks. Ensure all tests pass before pushing.

### CI/CD Pipeline
Tests are run in the CI/CD pipeline. All tests must pass before merging to main.

## Common Testing Scenarios

### Testing API Calls
```typescript
import { vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';

vi.mock('@tauri-apps/api/core');

it('should fetch data from API', async () => {
  const mockData = { id: 1, name: 'Test' };
  vi.mocked(invoke).mockResolvedValueOnce(mockData);
  
  const result = await getSettings();
  expect(result).toEqual(mockData);
});
```

### Testing State Updates
```typescript
import { renderHook, act } from '@testing-library/react';
import { useAppStore } from '../appStore';

it('should update state', () => {
  const { result } = renderHook(() => useAppStore());
  
  act(() => {
    result.current.setCurrentView('dashboard');
  });
  
  expect(result.current.currentView).toBe('dashboard');
});
```

### Testing Error Boundaries
```typescript
import { render, screen } from '@testing-library/react';
import { ErrorBoundary } from '../ErrorBoundary';

const ThrowError = () => {
  throw new Error('Test error');
};

it('should catch errors', () => {
  render(
    <ErrorBoundary>
      <ThrowError />
    </ErrorBoundary>
  );
  
  expect(screen.getByText(/应用出错/)).toBeInTheDocument();
});
```

## Resources

- [Vitest Documentation](https://vitest.dev/)
- [React Testing Library](https://testing-library.com/react)
- [Testing Best Practices](https://kentcdodds.com/blog/common-mistakes-with-react-testing-library)
