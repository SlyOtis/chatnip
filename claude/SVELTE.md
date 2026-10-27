# Svelte 5 Implementation Guidelines

## Overview
Svelte 5 introduces runes, a new reactive system that fundamentally changes how state and reactivity work in Svelte components.

## Runes System
Runes are special syntax elements with a "$" prefix that control the Svelte compiler.

### Key Characteristics
- Look like functions but are built-in language keywords
- Cannot be assigned to variables or passed as function arguments
- Provide fine-grained control over reactivity

## Core Runes

### $state - Reactive State
Creates reactive state that triggers updates when changed.

```javascript
let count = $state(0);
let user = $state({ name: 'John', age: 30 });

function increment() {
    count++; // Automatically triggers reactivity
}
```

### $derived - Computed Values
Creates derived state that automatically updates when dependencies change.

```javascript
let firstName = $state('John');
let lastName = $state('Doe');

let fullName = $derived(firstName + ' ' + lastName);
let uppercaseName = $derived(fullName.toUpperCase());
```

### $effect - Side Effects
Manages side effects that should run when dependencies change.

```javascript
let count = $state(0);

$effect(() => {
    console.log('Count changed to:', count);
    document.title = `Count: ${count}`;
});
```

### $props - Component Props
Defines component properties with optional destructuring and defaults.

```javascript
let { name, age = 25, ...rest } = $props();

// Usage in component
<script>
let { title, items = [], onSelect } = $props();
</script>
```

### $bindable - Two-way Binding
Creates bindable props for two-way data flow.

```javascript
// In child component
let { value = $bindable() } = $props();

// Parent can bind to it
<ChildComponent bind:value={myValue} />
```

### $inspect - Development Tool
Inspects state changes for debugging (development only).

```javascript
let data = $state({ items: [] });

$inspect(data); // Logs when data changes
```

### $host - Component Reference
Gets reference to the component's host element.

```javascript
let host = $host();

$effect(() => {
    host.style.backgroundColor = 'red';
});
```

## Component Patterns

### Basic Component with Runes
```svelte
<script>
let count = $state(0);
let doubled = $derived(count * 2);

function increment() {
    count++;
}

$effect(() => {
    if (count > 10) {
        console.log('Count is getting high!');
    }
});
</script>

<button onclick={increment}>
    Count: {count} (doubled: {doubled})
</button>
```

### Component with Props
```svelte
<script>
let { 
    title, 
    items = [], 
    onItemClick,
    disabled = false 
} = $props();

let selectedIndex = $state(-1);
let selectedItem = $derived(
    selectedIndex >= 0 ? items[selectedIndex] : null
);

function handleClick(item, index) {
    if (!disabled) {
        selectedIndex = index;
        onItemClick?.(item, index);
    }
}
</script>

<h2>{title}</h2>
<ul>
    {#each items as item, index}
        <li 
            class:selected={index === selectedIndex}
            onclick={() => handleClick(item, index)}
        >
            {item.name}
        </li>
    {/each}
</ul>
```

### Store Pattern with Runes
```javascript
// stores.js
function createUserStore() {
    let user = $state({ name: '', email: '', isLoggedIn: false });
    
    return {
        get user() { return user; },
        login(userData) {
            user.name = userData.name;
            user.email = userData.email;
            user.isLoggedIn = true;
        },
        logout() {
            user = { name: '', email: '', isLoggedIn: false };
        }
    };
}

export const userStore = createUserStore();
```

## Best Practices

### State Management
- Use `$state` for component-local reactive data
- Use `$derived` for computed values instead of reactive statements
- Use `$effect` sparingly, prefer derived state when possible

### Component Design
```svelte
<script>
// Props destructuring with defaults
let { 
    title = 'Default Title',
    items = [],
    maxItems = 10,
    onSelect
} = $props();

// Local state
let searchTerm = $state('');
let isLoading = $state(false);

// Derived state
let filteredItems = $derived(
    items.filter(item => 
        item.name.toLowerCase().includes(searchTerm.toLowerCase())
    ).slice(0, maxItems)
);

// Side effects
$effect(() => {
    if (searchTerm) {
        isLoading = true;
        // Simulate async search
        setTimeout(() => {
            isLoading = false;
        }, 300);
    }
});

function selectItem(item) {
    onSelect?.(item);
}
</script>
```

### Event Handling
```svelte
<script>
let formData = $state({ name: '', email: '' });
let errors = $state({});

let isValid = $derived(
    formData.name.length > 0 && formData.email.includes('@')
);

function handleSubmit(event) {
    event.preventDefault();
    if (isValid) {
        // Submit form
        console.log('Submitting:', formData);
    }
}

function updateField(field, value) {
    formData[field] = value;
    // Clear error when user starts typing
    if (errors[field]) {
        errors[field] = '';
    }
}
</script>

<form onsubmit={handleSubmit}>
    <input 
        bind:value={formData.name}
        oninput={(e) => updateField('name', e.target.value)}
        placeholder="Name"
    />
    
    <input 
        bind:value={formData.email}
        oninput={(e) => updateField('email', e.target.value)}
        placeholder="Email"
        type="email"
    />
    
    <button type="submit" disabled={!isValid}>
        Submit
    </button>
</form>
```

### Async Patterns
```svelte
<script>
let data = $state(null);
let loading = $state(false);
let error = $state(null);

async function fetchData() {
    loading = true;
    error = null;
    
    try {
        const response = await fetch('/api/data');
        data = await response.json();
    } catch (e) {
        error = e.message;
    } finally {
        loading = false;
    }
}

// Auto-fetch on mount
$effect(() => {
    fetchData();
});
</script>

{#if loading}
    <p>Loading...</p>
{:else if error}
    <p>Error: {error}</p>
{:else if data}
    <pre>{JSON.stringify(data, null, 2)}</pre>
{/if}
```

## Migration from Svelte 4
- Replace `let` reactive declarations with `$state`
- Replace `$:` reactive statements with `$derived`
- Replace lifecycle functions with `$effect`
- Update prop definitions to use `$props()`

## TypeScript Support
```typescript
<script lang="ts">
interface Item {
    id: number;
    name: string;
    completed: boolean;
}

interface Props {
    items: Item[];
    onToggle?: (item: Item) => void;
    title?: string;
}

let { items, onToggle, title = 'Todo List' }: Props = $props();

let completedCount = $derived(
    items.filter(item => item.completed).length
);
</script>
```

## Performance Tips
- Use `$derived` for expensive computations that depend on reactive state
- Avoid unnecessary `$effect` usage - prefer derived state
- Use `$inspect` only in development for debugging
- Minimize the scope of reactive state to reduce unnecessary updates