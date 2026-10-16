package testfixture;

import java.io.IOException;
import java.nio.file.*;
import java.util.*;
import testfixture.MockTypes.*;

/** Assert exporter outputs and side-effect order using only invented values. */
public final class ExporterChecks {
    private ExporterChecks() {}

    private static GhidraScript<Exception> script(String name, Path root) throws ReflectiveOperationException {
        GhidraScript<Exception> script = (GhidraScript<Exception>) Class.forName(name).getDeclaredConstructor().newInstance();
        script.args = new String[] {root.toString()};
        Function first = new Function(1);
        first.callers.add(new Function(3));
        first.callers.add(new Function(2));
        first.callees.add(new Function(4));
        Function second = new Function(2);
        second.completed = false;
        script.currentProgram.functions.functions.addAll(List.of(first, second));
        script.currentProgram.listing.data.addAll(List.of(
            new Data(10, false, "ignored"), new Data(11, true, null),
            new Data(12, true, "a\tb\nc\rd"), new Data(13, true, "")
        ));
        script.currentProgram.references.references.addAll(List.of(
            new Reference(1, 10), new Reference(1, 11), new Reference(1, 12),
            new Reference(2, 12), new Reference(2, 12), new Reference(9, 12),
            new Reference(2, 99)
        ));
        DecompInterface.events.clear();
        return script;
    }

    private static String read(Path root, String file) throws IOException {
        return Files.readString(root.resolve(file)).replace("\r\n", "\n");
    }

    private static void assertValue(Object actual, Object expected) {
        if (!Objects.equals(actual, expected)) {
            throw new AssertionError("Expected " + expected + "; got " + actual);
        }
    }

    private static String header(String address) {
        return "// synthetic_" + address + " at " + address + " size 2\n";
    }

    private static void checkAll(Path root) throws Exception {
        GhidraScript<Exception> script = script("DecompileAll", root);
        Path dir = Files.createDirectories(root.resolve("decomp_all"));
        Files.writeString(dir.resolve("1.c"), "existing");
        script.run();
        assertValue(read(root, "decomp_all/1.c"), "existing");
        assertValue(read(root, "decomp_all/2.c"), header("2") + "// callers: \n// callees: \n// decompilation failed: synthetic failure\n");
        assertValue(DecompInterface.events, List.of("code:true", "style:decompile", "open", "decompile:2:90", "dispose"));
        assertValue(script.messages, List.of("decompilation failures: 1", "decompiled 2 functions into " + dir));
    }

    private static Path list(Path root) throws IOException {
        Files.createDirectories(root);
        Path input = root.resolve("input.txt");
        Files.writeString(input, "\n # comment\n 0x1 \n9\n2\n");
        return input;
    }

    private static void checkList(Path root) throws Exception {
        GhidraScript<Exception> script = script("DecompileList", root);
        script.args = new String[] {list(root).toString(), root.toString(), "normalize"};
        script.run();
        assertValue(read(root, "decomp_normalize/1.c"), header("1") + "// callers: 2,3\n// callees: 4\nsynthetic payload\n");
        assertValue(read(root, "decomp_normalize/2.c"), header("2") + "// callers: \n// callees: \n// decompilation failed: synthetic failure\n");
        assertValue(DecompInterface.events, List.of("code:true", "style:normalize", "open", "decompile:1:120", "decompile:2:120", "dispose"));
        assertValue(script.messages, List.of("no function at 9", "decompilation failures: 1", "decompilation written to " + root.resolve("decomp_normalize")));
    }

    private static void checkInventory(Path root) throws Exception {
        GhidraScript<Exception> script = script("ExportInventory", root);
        script.run();
        String columns = String.join("\t", "address", "size", "name", "callers", "callees", "string_refs", "first_string");
        assertValue(read(root, "inventory.tsv"), columns + "\n1\t2\tsynthetic_1\t2\t1\t4\tb\n2\t2\tsynthetic_2\t0\t0\t4\tb\n");
        assertValue(script.messages, List.of("inventory written to " + root));
    }

    private static void checkListing(Path root) throws Exception {
        GhidraScript<Exception> script = script("ExportListing", root);
        script.args = new String[] {list(root).toString(), root.toString()};
        script.run();
        assertValue(read(root, "listing/1.asm"), "; synthetic_1 at 1 size 2\n1  synthetic instruction\n1  synthetic instruction\n");
        assertValue(read(root, "listing/2.asm"), "; synthetic_2 at 2 size 2\n1  synthetic instruction\n1  synthetic instruction\n");
        assertValue(script.messages, List.of("no function at 9", "listing written to " + root.resolve("listing")));
    }

    private static void checkStrings(Path root) throws Exception {
        GhidraScript<Exception> script = script("ExportStrings", root);
        script.run();
        String columns = String.join("\t", "address", "length", "functions", "text");
        assertValue(read(root, "strings.tsv"), columns + "\nc\t7\t1,2\ta b c d\nd\t0\t\t\n");
        assertValue(script.messages, List.of("strings written to " + root));
    }

    private static void checkCancel(Path root) throws Exception {
        for (String name : List.of("DecompileAll", "DecompileList", "ExportInventory", "ExportListing", "ExportStrings")) {
            GhidraScript<Exception> script = script(name, root.resolve(name));
            if (name.equals("DecompileList") || name.equals("ExportListing")) {
                script.args = new String[] {list(root).toString(), root.resolve(name).toString()};
            }
            script.monitor.cancelled = true;
            try {
                script.run();
                throw new AssertionError("expected cancellation from " + name);
            } catch (IOException error) {
                assertValue(error.getMessage(), "cancelled: the output is incomplete");
            }
        }
    }

    private static void checkException(Path root) throws Exception {
        for (String name : List.of("DecompileAll", "DecompileList")) {
            GhidraScript<Exception> script = script(name, root.resolve(name));
            if (name.equals("DecompileList")) {
                script.args = new String[] {list(root).toString(), root.resolve(name).toString()};
            }
            script.currentProgram.functions.functions.get(0).throwOnDecompile = true;
            try {
                script.run();
                throw new AssertionError("expected synthetic exception");
            } catch (IllegalStateException error) {
                assertValue(error.getMessage(), "synthetic exception");
                assertValue(DecompInterface.events.get(DecompInterface.events.size() - 1), "dispose");
            }
        }
    }

    private static void checkRefuse(Path root) throws Exception {
        for (String name : List.of("DecompileAll", "ExportInventory", "ExportStrings")) {
            GhidraScript<Exception> script = script(name, root);
            try {
                script.run();
                throw new AssertionError("expected output refusal");
            } catch (IOException error) {
                assertValue(error.getMessage(), "refusing to write outside the analysis root (a directory named re): " + root);
            }
            assertValue(DecompInterface.events, List.of());
        }
    }

    public static void main(String[] args) throws Exception {
        Path root = Path.of(args[1]).toAbsolutePath().resolve("re");
        switch (args[0]) {
            case "all" -> checkAll(root);
            case "list" -> checkList(root);
            case "inventory" -> checkInventory(root);
            case "listing" -> checkListing(root);
            case "strings" -> checkStrings(root);
            case "cancel" -> checkCancel(root);
            case "exception" -> checkException(root);
            case "refuse" -> checkRefuse(root.getParent());
            default -> throw new IllegalArgumentException(args[0]);
        }
    }
}
