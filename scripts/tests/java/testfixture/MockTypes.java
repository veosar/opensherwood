package testfixture;

import java.util.*;

/** Minimal synthetic doubles; no Ghidra or original program is loaded. */
public final class MockTypes {
    private MockTypes() {}

    public abstract static class GhidraScript<E extends Exception> {
        public final Program currentProgram = new Program();
        public final Monitor monitor = new Monitor();
        public String[] args = {};
        public final List<String> messages = new ArrayList<>();
        public String[] getScriptArgs() { return args; }
        public void println(String text) { messages.add(text); }
        public abstract void run() throws E;
    }

    public static class Monitor {
        public boolean cancelled;
        public boolean isCancelled() { return cancelled; }
    }

    public static class Address {
        public final long value;
        public Address(long value) { this.value = value; }
        @Override public String toString() { return Long.toHexString(value); }
    }

    public static class AddressFactory {
        public AddressFactory getDefaultAddressSpace() { return this; }
        public Address getAddress(long value) { return new Address(value); }
    }

    public static class Body {
        public long getNumAddresses() { return 2; }
        public AddressIterator getAddresses(boolean forward) {
            requireForward(forward);
            return new AddressIterator(List.of(new Address(1), new Address(2)));
        }
    }

    public static class Function {
        public final Address address;
        public final Set<Function> callers = new LinkedHashSet<>();
        public final Set<Function> callees = new LinkedHashSet<>();
        public boolean completed = true;
        public boolean throwOnDecompile;
        public Function(long address) { this.address = new Address(address); }
        public Address getEntryPoint() { return address; }
        public Body getBody() { return new Body(); }
        public String getName() { return "synthetic_" + address; }
        public Set<Function> getCallingFunctions(Monitor monitor) {
            Objects.requireNonNull(monitor);
            return callers;
        }
        public Set<Function> getCalledFunctions(Monitor monitor) {
            Objects.requireNonNull(monitor);
            return callees;
        }
    }

    public static class FunctionManager {
        public final List<Function> functions = new ArrayList<>();
        public FunctionIterator getFunctions(boolean forward) {
            requireForward(forward);
            return new FunctionIterator(functions);
        }
        public Function getFunctionContaining(Address address) {
            return functions.stream().filter(f -> f.address.value == address.value).findFirst().orElse(null);
        }
    }

    public static class Data {
        public final Address address;
        public final boolean stringValue;
        public final Object value;
        public Data(long address, boolean stringValue, Object value) {
            this.address = new Address(address);
            this.stringValue = stringValue;
            this.value = value;
        }
        public Address getAddress() { return address; }
        public boolean hasStringValue() { return stringValue; }
        public Object getValue() { return value; }
    }

    public static class Reference {
        public final Address from;
        public final Address to;
        public Reference(long from, long to) { this.from = new Address(from); this.to = new Address(to); }
        public Address getFromAddress() { return from; }
        public Address getToAddress() { return to; }
    }

    public static class ReferenceManager {
        public final List<Reference> references = new ArrayList<>();
        public List<Reference> getReferencesFrom(Address address) {
            return references.stream().filter(r -> r.from.value == address.value).toList();
        }
        public List<Reference> getReferencesTo(Address address) {
            return references.stream().filter(r -> r.to.value == address.value).toList();
        }
    }

    public static class Instruction {
        public Address getAddress() { return new Address(1); }
        @Override public String toString() { return "synthetic instruction"; }
    }

    public static class Listing {
        public final List<Data> data = new ArrayList<>();
        public Data getDefinedDataAt(Address address) {
            return data.stream().filter(d -> d.address.value == address.value).findFirst().orElse(null);
        }
        public DataIterator getDefinedData(boolean forward) {
            requireForward(forward);
            return new DataIterator(data);
        }
        public InstructionIterator getInstructions(Body body, boolean forward) {
            Objects.requireNonNull(body);
            requireForward(forward);
            return new InstructionIterator(List.of(new Instruction(), new Instruction()));
        }
    }

    public static class Program {
        public final FunctionManager functions = new FunctionManager();
        public final ReferenceManager references = new ReferenceManager();
        public final Listing listing = new Listing();
        public FunctionManager getFunctionManager() { return functions; }
        public ReferenceManager getReferenceManager() { return references; }
        public Listing getListing() { return listing; }
        public AddressFactory getAddressFactory() { return new AddressFactory(); }
    }

    public static class Values<T> {
        private final Iterator<T> iterator;
        public Values(List<T> values) { iterator = values.iterator(); }
        public boolean hasNext() { return iterator.hasNext(); }
        public T next() { return iterator.next(); }
    }
    public static class FunctionIterator extends Values<Function> {
        public FunctionIterator(List<Function> values) { super(values); }
    }
    public static class AddressIterator extends Values<Address> {
        public AddressIterator(List<Address> values) { super(values); }
    }
    public static class DataIterator extends Values<Data> {
        public DataIterator(List<Data> values) { super(values); }
    }
    public static class InstructionIterator extends Values<Instruction> {
        public InstructionIterator(List<Instruction> values) { super(values); }
    }

    public static class DecompInterface {
        static final List<String> events = new ArrayList<>();
        public void toggleCCode(boolean value) { events.add("code:" + value); }
        public void setSimplificationStyle(String value) { events.add("style:" + value); }
        public boolean openProgram(Program program) {
            Objects.requireNonNull(program);
            events.add("open");
            return true;
        }
        public String getLastMessage() { return "synthetic failure"; }
        public DecompileResults decompileFunction(Function function, int timeout, Monitor monitor) {
            Objects.requireNonNull(monitor);
            events.add("decompile:" + function.address + ":" + timeout);
            if (function.throwOnDecompile) throw new IllegalStateException("synthetic exception");
            return new DecompileResults(function.completed);
        }
        public void dispose() { events.add("dispose"); }
    }

    public static class DecompileResults {
        private final boolean completed;
        public DecompileResults(boolean completed) { this.completed = completed; }
        public boolean decompileCompleted() { return completed; }
        public DecompileResults getDecompiledFunction() { return this; }
        public String getC() { return "synthetic payload"; }
        public String getErrorMessage() { return "synthetic failure"; }
    }

    private static void requireForward(boolean forward) {
        if (!forward) throw new AssertionError("expected forward iteration");
    }
}
