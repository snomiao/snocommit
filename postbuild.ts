import fsp from "node:fs/promises";
import { execAndLog } from "async-exec";
import "p-map";
import pMap from "p-map";
import commitTypes from "./src/commitTypes";

await pMap(commitTypes, async (type) => {
  const targetFile = `dist/cli-${type}.js`;
  const _r = await execAndLog(`npm pkg set bin.${type}="${targetFile}"`);
  return await fsp
    .copyFile("dist/cli.js", targetFile)
    .then((_e) => console.log(`cli-${type} setup done`));
});

console.log("post build done");
