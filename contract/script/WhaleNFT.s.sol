// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.13;

import {Script, console} from "forge-std/Script.sol";
import {WhaleNFT} from "../src/WhaleNFT.sol";

contract WhaleNFTScript is Script {
    WhaleNFT public whale;

    function setUp() public {}

    function run() public {
        vm.startBroadcast();

        whale = new WhaleNFT();

        vm.stopBroadcast();
    }
}
