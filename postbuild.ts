import fsp from "node:fs/promises";
import pMap from "p-map";
import commitTypes from "./src/commitTypes";

// Read and update package.json
const pkgPath = "package.json";
const pkgContent = await fsp.readFile(pkgPath, "utf-8");
const pkg = JSON.parse(pkgContent);

// Initialize bin if it doesn't exist
if (!pkg.bin) {
  pkg.bin = {};
}

// Copy CLI files and update bin entries
await pMap(commitTypes, async (type) => {
  const targetFile = `dist/cli-${type}.js`;
  pkg.bin[type] = targetFile;
  await fsp.copyFile("dist/cli.js", targetFile);
  console.log(`cli-${type} setup done`);
});

// Write updated package.json
await fsp.writeFile(pkgPath, `${JSON.stringify(pkg, null, 2)}\n`, "utf-8");

console.log("post build done");
